//! Connected advisory D6 decisions. Missing private ancestry stays server-decided.
use crate::diff_apply::{ClassifiedRow, RowClass};
use serde::{Deserialize, Serialize};
use spellkave_schema::{CapabilityDefinitionRow, Manifest, RoleAggregationRow, RoleDefinitionRow};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Deserialize)]
pub struct Grant {
    pub capability_id: String,
    pub extent_kind_ref: String,
    pub extent_ref: String,
    pub expires_at_tick: Option<u64>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Binding {
    pub binding_id: String,
    pub bundle_id: String,
    pub role_id: String,
    pub extent_kind_ref: String,
    pub extent_ref: String,
    pub realised: bool,
}

#[derive(Clone, Debug, Deserialize)]
pub struct AuthoritySnapshot {
    pub publisher: bool,
    pub scoped: bool,
    pub complete: bool,
    pub tick: u64,
    pub grants: Vec<Grant>,
    pub role_definitions: Vec<RoleDefinitionRow>,
    pub role_aggregations: Vec<RoleAggregationRow>,
    pub capability_definitions: Vec<CapabilityDefinitionRow>,
    pub bindings: Vec<Binding>,
    #[serde(default)]
    pub extent_shapes: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct AuthorityVerdict {
    pub kind: String,
    pub key: String,
    pub code: Option<String>,
    pub reason: String,
    pub binding_id: Option<String>,
    pub role_id: Option<String>,
    pub extent_kind_ref: Option<String>,
    pub extent_ref: Option<String>,
}

fn verdict(kind: &str, key: &str, code: Option<&str>, reason: &str) -> AuthorityVerdict {
    AuthorityVerdict {
        kind: kind.into(),
        key: key.into(),
        code: code.map(str::to_owned),
        reason: reason.into(),
        binding_id: None,
        role_id: None,
        extent_kind_ref: None,
        extent_ref: None,
    }
}

/// Equal references decide; unequal graph references may conceal an ancestor relation.
fn covers(grant: &Grant, kind: &str, reference: &str) -> Option<bool> {
    if grant.extent_ref.is_empty() || reference.is_empty() {
        return Some(false);
    }
    let grant_kind = if grant.extent_kind_ref.is_empty() {
        "place_graph"
    } else {
        &grant.extent_kind_ref
    };
    let kind = if kind.is_empty() { "place_graph" } else { kind };
    if kind == "place_graph" && matches!(grant_kind, "place_graph" | "bundle") {
        if grant_kind == "place_graph" && grant.extent_ref == reference {
            return Some(true);
        }
        return None;
    }
    Some(grant_kind == kind && grant.extent_ref == reference && matches!(kind, "party" | "bundle"))
}

fn any(values: impl Iterator<Item = Option<bool>>) -> Option<bool> {
    let values: Vec<_> = values.collect();
    if values.contains(&Some(true)) {
        Some(true)
    } else if values.contains(&None) {
        None
    } else {
        Some(false)
    }
}

fn has(snapshot: &AuthoritySnapshot, id: &str, kind: &str, reference: &str) -> Option<bool> {
    any(snapshot
        .grants
        .iter()
        .filter(|g| {
            g.capability_id == id
                && g.expires_at_tick
                    .is_none_or(|expiry| snapshot.tick < expiry)
        })
        .map(|g| {
            let mut resolved = g.clone();
            resolved.extent_kind_ref = snapshot
                .extent_shapes
                .get(&g.extent_kind_ref)
                .cloned()
                .unwrap_or_else(|| g.extent_kind_ref.clone());
            covers(
                &resolved,
                snapshot
                    .extent_shapes
                    .get(kind)
                    .map(String::as_str)
                    .unwrap_or(kind),
                reference,
            )
        }))
}

fn all(snapshot: &AuthoritySnapshot, ids: &[String], kind: &str, reference: &str) -> Option<bool> {
    if ids.is_empty() {
        return Some(false);
    }
    let values: Vec<_> = ids
        .iter()
        .map(|id| has(snapshot, id, kind, reference))
        .collect();
    if values.contains(&Some(false)) {
        Some(false)
    } else if values.contains(&None) {
        None
    } else {
        Some(true)
    }
}

fn bind(snapshot: &AuthoritySnapshot, binding: &Binding, ids: &[String]) -> Option<bool> {
    if ids
        .iter()
        .any(|id| matches!(id.as_str(), "frontier.apply_manifest"))
    {
        return Some(false);
    }
    any([
        has(
            snapshot,
            &format!("frontier.bind.{}", binding.role_id),
            &binding.extent_kind_ref,
            &binding.extent_ref,
        ),
        all(snapshot, ids, &binding.extent_kind_ref, &binding.extent_ref),
    ]
    .into_iter())
}

pub fn authority_verdicts(
    snapshot: Option<&AuthoritySnapshot>,
    plan: &Manifest,
    rows: &[ClassifiedRow],
    prune: bool,
) -> Vec<AuthorityVerdict> {
    let writes: Vec<_> = rows
        .iter()
        .filter(|row| {
            matches!(row.class, RowClass::Create | RowClass::Configure)
                || (prune && row.class == RowClass::Prune)
        })
        .collect();
    let privileged = spellkave_validate::refs::privileged_templates::rows(plan);
    let sensitive = |kind: &str| {
        matches!(
            kind,
            "role_definitions" | "role_aggregations" | "role_bindings" | "capability_definitions"
        ) || privileged.iter().any(|row| row.kind == kind)
    };
    let Some(snapshot) = snapshot else {
        return writes
            .iter()
            .filter(|row| sensitive(&row.kind) || (prune && row.class == RowClass::Prune))
            .map(|row| verdict(&row.kind, &row.key, None, "server decides"))
            .collect();
    };
    if snapshot.publisher {
        return Vec::new();
    }
    if !snapshot.complete {
        return writes
            .iter()
            .filter(|row| sensitive(&row.kind) || row.class == RowClass::Prune)
            .map(|row| verdict(&row.kind, &row.key, None, "server decides"))
            .collect();
    }
    let mut definitions = snapshot.role_definitions.clone();
    let mut aggregations = snapshot.role_aggregations.clone();
    for row in &plan.retractions {
        if row.row_kind == "role_definition" {
            definitions.retain(|old| old.role_id != row.authored_key);
        }
        if row.row_kind == "role_aggregation" {
            aggregations.retain(|old| {
                spellkave_schema::row_identity::row_key_of(
                    "role_aggregations",
                    &serde_json::to_value(old).expect("row serializes"),
                )
                .as_deref()
                    != Some(row.authored_key.as_str())
            });
        }
    }
    for row in &plan.role_definitions {
        definitions.retain(|old| old.role_id != row.role_id);
        definitions.push(row.clone());
    }
    for row in &plan.role_aggregations {
        aggregations.retain(|old| old.role_id != row.role_id || old.bundle_id != row.bundle_id);
        aggregations.push(row.clone());
    }
    let merged = |role: &str| {
        spellkave_validate::merged_role_capability_ids(role, &definitions, &aggregations)
    };
    let previous = |role: &str| {
        spellkave_validate::merged_role_capability_ids(
            role,
            &snapshot.role_definitions,
            &snapshot.role_aggregations,
        )
    };
    let mut result = Vec::new();
    let mut affected = BTreeSet::new();
    for row in &writes {
        if let Some(authority) = privileged
            .iter()
            .find(|gate| gate.kind == row.kind && gate.key == row.key)
        {
            let admits = any([
                all(
                    snapshot,
                    &authority.capability_ids,
                    "bundle",
                    &authority.bundle_id,
                ),
                has(
                    snapshot,
                    "frontier.escalate",
                    "bundle",
                    &authority.bundle_id,
                ),
            ]
            .into_iter());
            match admits {
                Some(true) => {}
                Some(false) => result.push(verdict(
                    &row.kind,
                    &row.key,
                    Some("MF-APPLY-039"),
                    "missing_capability",
                )),
                None => result.push(verdict(&row.kind, &row.key, None, "server decides")),
            }
        }
        let mut required = Vec::new();
        let mut row_ids = Vec::new();
        let mut role = None;
        if row.kind == "role_definitions" {
            let new = plan.role_definitions.iter().find(|r| r.role_id == row.key);
            let old = snapshot
                .role_definitions
                .iter()
                .find(|r| r.role_id == row.key);
            if new == old && new.is_some() {
                continue;
            }
            row_ids.extend(
                new.into_iter()
                    .flat_map(|row| row.capability_ids.iter().cloned()),
            );
            role = new
                .map(|r| r.role_id.as_str())
                .or_else(|| old.map(|r| r.role_id.as_str()));
            if row.class != RowClass::Prune {
                if let Some(role) = role {
                    required = merged(role);
                }
            }
            if let Some(old) = old {
                required.extend(
                    old.capability_ids
                        .iter()
                        .filter(|id| new.is_none_or(|new| !new.capability_ids.contains(id)))
                        .cloned(),
                );
            }
        } else if row.kind == "role_aggregations" {
            // Authoring and server storage share the canonical JSON composite.
            let key: Vec<String> = serde_json::from_str(&row.key).unwrap_or_default();
            let new = plan
                .role_aggregations
                .iter()
                .find(|r| key == [r.role_id.clone(), r.bundle_id.clone()]);
            let old = snapshot
                .role_aggregations
                .iter()
                .find(|r| key == [r.role_id.clone(), r.bundle_id.clone()]);
            if new == old && new.is_some() {
                continue;
            }
            row_ids.extend(
                new.into_iter()
                    .flat_map(|row| row.capability_ids.iter().cloned()),
            );
            role = new
                .map(|r| r.role_id.as_str())
                .or_else(|| old.map(|r| r.role_id.as_str()));
            if row.class != RowClass::Prune {
                if let Some(role) = role {
                    required = merged(role);
                }
            }
            if let Some(old) = old {
                required.extend(
                    old.capability_ids
                        .iter()
                        .filter(|id| new.is_none_or(|new| !new.capability_ids.contains(id)))
                        .cloned(),
                );
            }
        } else if row.kind == "capability_definitions" && row.key.starts_with("frontier.") {
            if !snapshot
                .capability_definitions
                .iter()
                .any(|r| r.capability_id == row.key)
            {
                result.push(verdict(
                    &row.kind,
                    &row.key,
                    Some("MF-APPLY-035"),
                    "reserved_definition_root",
                ));
                continue;
            }
            if plan
                .capability_definitions
                .iter()
                .find(|r| r.capability_id == row.key)
                == snapshot
                    .capability_definitions
                    .iter()
                    .find(|r| r.capability_id == row.key)
            {
                continue;
            }
            required.push(row.key.clone());
            row_ids.push(row.key.clone());
        }
        if let Some(role) = role {
            affected.insert(role.to_owned());
            if merged(role).contains(&"frontier.pace_time".into())
                && !previous(role).contains(&"frontier.pace_time".into())
            {
                result.push(verdict(
                    &row.kind,
                    &row.key,
                    Some("MF-APPLY-035"),
                    "clock_root",
                ));
                continue;
            }
        }
        if role.is_some() || !required.is_empty() {
            if snapshot.scoped {
                let bundle = row.bundle_id.as_deref().unwrap_or("");
                let admits = any([
                    all(snapshot, &required, "bundle", bundle),
                    has(snapshot, "frontier.escalate", "bundle", bundle),
                ]
                .into_iter());
                match admits {
                    Some(true) => {}
                    Some(false) => result.push(verdict(
                        &row.kind,
                        &row.key,
                        Some("MF-APPLY-045"),
                        "missing_capability",
                    )),
                    None => result.push(verdict(&row.kind, &row.key, None, "server decides")),
                }
            } else if row_ids.iter().any(|id| id.starts_with("frontier.")) {
                result.push(verdict(
                    &row.kind,
                    &row.key,
                    Some("MF-APPLY-035"),
                    "reserved_definition_root",
                ));
            }
        }
        if snapshot.scoped
            && row.class == RowClass::Prune
            && !matches!(row.kind.as_str(), "role_bindings" | "actor_capabilities")
        {
            // A bundle grant is sufficient only for an unplaced row; placement needs server data.
            let has_prune = snapshot.grants.iter().any(|g| {
                g.capability_id == "frontier.prune"
                    && g.expires_at_tick
                        .is_none_or(|expiry| snapshot.tick < expiry)
            });
            result.push(verdict(
                &row.kind,
                &row.key,
                (!has_prune).then_some("MF-APPLY-046"),
                if has_prune {
                    "server decides"
                } else {
                    "missing_capability"
                },
            ));
        }
    }
    let incoming: BTreeMap<_, _> = plan
        .role_bindings
        .iter()
        .map(|row| (row.binding_id.as_str(), row))
        .collect();
    for binding in &snapshot.bindings {
        if !binding.realised
            || !affected.contains(&binding.role_id)
            || previous(&binding.role_id) == merged(&binding.role_id)
            || plan.retractions.iter().any(|r| {
                matches!(r.row_kind.as_str(), "role_binding" | "role_bindings")
                    && r.authored_key == binding.binding_id
            })
            || incoming
                .get(binding.binding_id.as_str())
                .is_some_and(|new| new.role_id != binding.role_id)
        {
            continue;
        }
        let old = bind(snapshot, binding, &previous(&binding.role_id));
        let new = bind(snapshot, binding, &merged(&binding.role_id));
        let admitted = if old == Some(false) || new == Some(false) {
            Some(false)
        } else if old.is_none() || new.is_none() {
            None
        } else {
            Some(true)
        };
        if admitted != Some(true) {
            let mut finding = verdict(
                "role_bindings",
                &binding.binding_id,
                (admitted == Some(false)).then_some("MF-APPLY-036"),
                if admitted.is_none() {
                    "server decides"
                } else {
                    "realised_binding_impact"
                },
            );
            finding.binding_id = Some(binding.binding_id.clone());
            finding.role_id = Some(binding.role_id.clone());
            finding.extent_kind_ref = Some(binding.extent_kind_ref.clone());
            finding.extent_ref = Some(binding.extent_ref.clone());
            result.push(finding);
        }
    }
    // Explicit binding updates and revocations also use the same pre-image grants.
    for row in writes.iter().filter(|row| row.kind == "role_bindings") {
        let old = snapshot.bindings.iter().find(|b| b.binding_id == row.key);
        let new = incoming.get(row.key.as_str()).map(|r| Binding {
            binding_id: r.binding_id.clone(),
            bundle_id: r.bundle_id.clone(),
            role_id: r.role_id.clone(),
            extent_kind_ref: r.extent_kind_ref.clone(),
            extent_ref: r.extent_ref.clone(),
            realised: true,
        });
        for (binding, ids) in old
            .filter(|b| b.realised || snapshot.scoped)
            .map(|b| (b, previous(&b.role_id)))
            .into_iter()
            .chain(
                new.as_ref()
                    .filter(|_| snapshot.scoped || old.is_some_and(|old| old.realised))
                    .map(|b| (b, merged(&b.role_id))),
            )
        {
            match bind(snapshot, binding, &ids) {
                Some(true) => {}
                Some(false) => result.push(verdict(
                    "role_bindings",
                    &row.key,
                    Some("MF-APPLY-036"),
                    "missing_capability",
                )),
                None => result.push(verdict("role_bindings", &row.key, None, "server decides")),
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    fn snapshot() -> AuthoritySnapshot {
        serde_json::from_value(serde_json::json!({
            "publisher": false, "scoped": true, "complete": true, "tick": 10,
            "grants": [
                {"capability_id":"custom/a", "extent_kind_ref":"bundle", "extent_ref":"own", "expires_at_tick":null},
                {"capability_id":"custom/b", "extent_kind_ref":"bundle", "extent_ref":"own", "expires_at_tick":null}
            ],
            "role_definitions":[{"role_id":"custom/scout","bundle_id":"own","display_name":"Scout","description":"Synthetic","capability_ids":["custom/a"]}],
            "role_aggregations":[], "capability_definitions":[],
            "bindings":[
                {"binding_id":"own/scout","bundle_id":"own","role_id":"custom/scout","extent_kind_ref":"bundle","extent_ref":"own","realised":true},
                {"binding_id":"foreign/scout","bundle_id":"foreign","role_id":"custom/scout","extent_kind_ref":"bundle","extent_ref":"foreign","realised":true}
            ]
        })).unwrap()
    }
    fn row(kind: &str, key: &str, class: RowClass) -> ClassifiedRow {
        ClassifiedRow {
            kind: kind.into(),
            key: key.into(),
            bundle_id: Some("own".into()),
            class,
            part: "root.toml".into(),
            from_part: None,
            hash: "synthetic".into(),
            live: None,
            world_changed: None,
        }
    }
    fn widened() -> Manifest {
        let mut plan = Manifest::default();
        let mut role = snapshot().role_definitions.remove(0);
        role.capability_ids.push("custom/b".into());
        plan.role_definitions.push(role);
        plan
    }
    #[test]
    fn privileged_template_verdicts_require_matching_bundle_grants_or_escalation() {
        let plan = spellkave_format::parse_manifest(
            r#"#:schema https://spellkave.dev/schema/world-manifest-1.0.0.json
schema_version = "1.0.0"
[[action_definitions]]
            action_id = 'own/privileged'
            bundle_id = 'own'
            required_capability = 'frontier.despawn'
            display_name = 'Privileged'
            on_success = ''
        "#,
        )
        .unwrap()
        .manifest;
        let rows = [row(
            "action_definitions",
            "own/privileged",
            RowClass::Create,
        )];
        let mut live = snapshot();
        assert_eq!(
            authority_verdicts(Some(&live), &plan, &rows, false)[0]
                .code
                .as_deref(),
            Some("MF-APPLY-039")
        );
        for id in ["frontier.despawn", "frontier.escalate"] {
            live.grants = vec![Grant {
                capability_id: id.into(),
                extent_kind_ref: "bundle".into(),
                extent_ref: "own".into(),
                expires_at_tick: None,
            }];
            assert!(authority_verdicts(Some(&live), &plan, &rows, false).is_empty());
            live.grants[0].extent_ref = "foreign".into();
            assert_eq!(
                authority_verdicts(Some(&live), &plan, &rows, false)[0]
                    .code
                    .as_deref(),
                Some("MF-APPLY-039")
            );
            live.grants[0].extent_ref = "own".into();
            live.grants[0].expires_at_tick = Some(live.tick);
            assert_eq!(
                authority_verdicts(Some(&live), &plan, &rows, false)[0]
                    .code
                    .as_deref(),
                Some("MF-APPLY-039")
            );
        }
        live.publisher = true;
        assert!(authority_verdicts(Some(&live), &plan, &rows, false).is_empty());
        assert_eq!(
            authority_verdicts(None, &plan, &rows, false)[0].reason,
            "server decides"
        );
    }

    #[test]
    fn affected_bindings_report_known_denial_and_keep_private_graph_ancestry_unknown() {
        let mut live = snapshot();
        let rows = [row("role_definitions", "custom/scout", RowClass::Configure)];
        let verdicts = authority_verdicts(Some(&live), &widened(), &rows, false);
        assert_eq!(verdicts.len(), 1);
        assert_eq!(verdicts[0].code.as_deref(), Some("MF-APPLY-036"));
        assert_eq!(verdicts[0].reason, "realised_binding_impact");
        assert_eq!(verdicts[0].binding_id.as_deref(), Some("foreign/scout"));
        assert!(!serde_json::to_string(&verdicts)
            .unwrap()
            .contains("subject"));
        live.bindings[1].extent_kind_ref = "place_graph".into();
        let unknown = authority_verdicts(Some(&live), &widened(), &rows, false);
        assert_eq!(unknown[0].reason, "server decides");
        assert_eq!(unknown[0].code, None);
        live.complete = false;
        assert!(authority_verdicts(Some(&live), &widened(), &rows, false)
            .iter()
            .all(|v| v.code.is_none()));
    }
    #[test]
    fn role_root_prune_and_expiry_codes_match_live_admission() {
        let mut live = snapshot();
        live.bindings.clear();
        live.grants.clear();
        let rows = [row("role_definitions", "custom/scout", RowClass::Configure)];
        assert_eq!(
            authority_verdicts(Some(&live), &widened(), &rows, false)[0]
                .code
                .as_deref(),
            Some("MF-APPLY-045")
        );
        live.grants.push(Grant {
            capability_id: "frontier.escalate".into(),
            extent_kind_ref: "bundle".into(),
            extent_ref: "own".into(),
            expires_at_tick: None,
        });
        assert!(authority_verdicts(Some(&live), &widened(), &rows, false).is_empty());
        let mut clock = widened();
        clock.role_definitions[0]
            .capability_ids
            .push("frontier.pace_time".into());
        let denied = authority_verdicts(Some(&live), &clock, &rows, false);
        assert_eq!(denied[0].code.as_deref(), Some("MF-APPLY-035"));
        assert_eq!(denied[0].reason, "clock_root");
        let prune = authority_verdicts(
            Some(&live),
            &Manifest::default(),
            &[row("object_state", "[\"body\",\"state\"]", RowClass::Prune)],
            true,
        );
        assert_eq!(prune[0].code.as_deref(), Some("MF-APPLY-046"));
        live.grants[0].expires_at_tick = Some(10);
        assert_eq!(
            authority_verdicts(Some(&live), &widened(), &rows, false)[0]
                .code
                .as_deref(),
            Some("MF-APPLY-045")
        );
    }
    #[test]
    fn legacy_ordinary_contribution_does_not_inherit_the_merged_reserved_row_rule() {
        let mut live = snapshot();
        live.scoped = false;
        live.bindings.clear();
        live.role_definitions[0].capability_ids = vec!["frontier.read_hidden".into()];
        let mut plan = Manifest::default();
        plan.role_aggregations.push(RoleAggregationRow {
            role_id: "custom/scout".into(),
            bundle_id: "own".into(),
            description: "Synthetic".into(),
            capability_ids: vec!["custom/a".into()],
        });
        assert!(authority_verdicts(
            Some(&live),
            &plan,
            &[row(
                "role_aggregations",
                "[\"custom/scout\",\"own\"]",
                RowClass::Create
            )],
            false
        )
        .is_empty());
    }
}
