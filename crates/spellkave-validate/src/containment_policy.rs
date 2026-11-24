//! Admission of deterministic containment predicates. The evaluator is the
//! shared DSL evaluator; this module only restricts its authored input surface.

use spellkave_dsl::{DslMacroDefinition, Expression, Op, SubjectRef};
use spellkave_schema::Manifest;
use std::collections::{BTreeMap, BTreeSet};

/// Existence/ownership facts supplied by an author target. This is validation
/// context, never authored rows: condition/policy DSL need not round-trip.
#[derive(Default, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ReferenceContext {
    pub bundle_dependencies: BTreeMap<String, BTreeSet<String>>,
    pub policies: BTreeMap<String, String>,
    pub states: BTreeMap<String, String>,
    pub tags: BTreeMap<String, String>,
    pub conditions: BTreeMap<String, String>,
    pub entities: BTreeMap<String, (String, String)>,
}

impl ReferenceContext {
    pub fn into_vocabulary(self) -> crate::refs::LiveVocabulary {
        crate::refs::LiveVocabulary {
            bundles: self.bundle_dependencies.keys().cloned().collect(),
            containment: Vocabulary {
                policies: self.policies,
                states: self.states,
                entities: self.entities,
                ..Default::default()
            },
            dm: crate::refs::DmVocabulary {
                bundle_dependencies: self.bundle_dependencies,
                tags: self.tags,
                conditions: self.conditions,
                ..Default::default()
            },
            ..Default::default()
        }
    }
}

#[derive(Default, Clone)]
pub struct Vocabulary {
    pub policies: BTreeMap<String, String>,
    pub states: BTreeMap<String, String>,
    /// Qualified authored entity key -> (owning bundle, definition).
    pub entities: BTreeMap<String, (String, String)>,
    pub definitions: BTreeMap<String, String>,
    pub instances: BTreeMap<String, String>,
}

impl Vocabulary {
    pub fn from_manifest(manifest: &Manifest) -> Self {
        Self {
            states: manifest
                .object_state_definitions
                .iter()
                .map(|r| (r.state_id.clone(), r.bundle_id.clone()))
                .chain(
                    manifest
                        .resource_definitions
                        .iter()
                        .map(|r| (r.resource_definition_id.clone(), r.bundle_id.clone())),
                )
                .collect(),
            policies: manifest
                .containment_policy_definition
                .iter()
                .map(|r| (r.policy_id.clone(), r.bundle_id.clone()))
                .collect(),
            entities: manifest
                .entities
                .iter()
                .map(|r| {
                    (
                        format!("{}:{}", r.bundle_id, r.entity_key),
                        (r.bundle_id.clone(), r.entity_def_id.clone()),
                    )
                })
                .collect(),
            definitions: manifest
                .entity_definition_containment_policy
                .iter()
                .map(|r| (r.entity_def_id.clone(), r.policy_id.clone()))
                .collect(),
            instances: manifest
                .entity_containment_policy
                .iter()
                .map(|r| (qualified(&r.bundle_id, &r.entity_key), r.policy_id.clone()))
                .collect(),
        }
    }
}

fn qualified(bundle: &str, key: &str) -> String {
    if key.contains(':') {
        key.into()
    } else {
        format!("{bundle}:{key}")
    }
}

fn diagnostic(path: &str, message: String, severity: crate::Severity) -> crate::Diagnostic {
    crate::Diagnostic {
        code: "MF-VAL-391".into(),
        severity,
        message,
        path: path.into(),
        span: None,
        hint: None,
        details: None,
    }
}

pub(crate) fn validate(
    manifest: &Manifest,
    applying: &Manifest,
    live: Option<&crate::refs::LiveVocabulary>,
    diags: &mut Vec<crate::Diagnostic>,
) {
    let mut vocabulary = live.map(|v| v.containment.clone()).unwrap_or_default();
    let incoming = Vocabulary::from_manifest(manifest);
    vocabulary.policies.extend(incoming.policies);
    vocabulary.states.extend(incoming.states);
    vocabulary.entities.extend(incoming.entities);
    vocabulary.definitions.extend(incoming.definitions);
    vocabulary.instances.extend(incoming.instances);
    let dependencies = live
        .map(|v| v.dm.bundle_dependencies.clone())
        .unwrap_or_default();
    let scope = crate::refs::BundleScope::build_with_live(manifest, &dependencies);
    let macros = crate::dsl_macros::dsl_macro_compiler_definitions(manifest);
    let mut tags = live.map(|v| v.dm.tags.clone()).unwrap_or_default();
    tags.extend(
        manifest
            .tag_value_definitions
            .iter()
            .map(|r| (r.tag.clone(), r.bundle_id.clone())),
    );
    let mut conditions = live.map(|v| v.dm.conditions.clone()).unwrap_or_default();
    conditions.extend(
        manifest
            .condition_definitions
            .iter()
            .map(|r| (r.condition_definition_id.clone(), r.bundle_id.clone())),
    );
    let known_bundles: BTreeSet<_> = manifest
        .bundles
        .iter()
        .map(|r| r.bundle_id.clone())
        .chain(live.into_iter().flat_map(|v| v.bundles.iter().cloned()))
        .collect();
    let mut seen = BTreeSet::new();
    for row in &applying.containment_policy_definition {
        if !known_bundles.contains(&row.bundle_id)
            || !row.policy_id.starts_with(&format!("{}/", row.bundle_id))
            || !seen.insert(&row.policy_id)
        {
            diags.push(diagnostic(
                "containment_policy_definition.policy_id",
                "unknown bundle, non-namespaced or duplicate policy id".into(),
                crate::Severity::Fatal,
            ));
        }
        for (field, source) in [
            ("exposure_predicate_source", &row.exposure_predicate_source),
            ("access_predicate_source", &row.access_predicate_source),
        ] {
            match compile(source, &macros) {
                Err(error) => diags.push(diagnostic(
                    &format!("containment_policy_definition.{field}"),
                    format!("policy '{}': {error}", row.policy_id),
                    crate::Severity::Fatal,
                )),
                Ok(expression) => {
                    // The admitted AST cannot read projections or derived formula sources,
                    // so its dependency graph contains only primary state/tag/condition leaves.
                    for node in &expression.nodes {
                        let reference = match &node.op {
                            Op::ReadInt(read) | Op::ReadBool(read) => {
                                Some((&read.field_id, vocabulary.states.get(&read.field_id)))
                            }
                            Op::HasTag(read) => Some((&read.tag, tags.get(&read.tag))),
                            Op::HasStatus(read) => Some((
                                &read.condition_definition_id,
                                conditions.get(&read.condition_definition_id),
                            )),
                            _ => None,
                        };
                        if let Some((id, owner)) = reference {
                            if !owner.is_some_and(|owner| scope.in_scope(&row.bundle_id, owner)) {
                                diags.push(diagnostic(
                                    &format!("containment_policy_definition.{field}"),
                                    format!(
                                        "policy '{}': unknown or out-of-closure input '{id}'",
                                        row.policy_id
                                    ),
                                    crate::Severity::Fatal,
                                ));
                            }
                        }
                    }
                }
            }
        }
    }
    let definitions: BTreeMap<_, _> = manifest
        .entity_definitions
        .iter()
        .map(|r| (r.entity_def_id.as_str(), r.bundle_id.as_str()))
        .collect();
    let mut keys = BTreeSet::new();
    for (kind, key, owner, policy, endpoint_owner) in applying
        .entity_definition_containment_policy
        .iter()
        .map(|r| {
            (
                "entity_definition_containment_policy",
                r.entity_def_id.clone(),
                &r.bundle_id,
                &r.policy_id,
                definitions
                    .get(r.entity_def_id.as_str())
                    .map(|b| b.to_string()),
            )
        })
        .chain(applying.entity_containment_policy.iter().map(|r| {
            let key = qualified(&r.bundle_id, &r.entity_key);
            let owner = vocabulary.entities.get(&key).map(|(b, _)| b.clone());
            (
                "entity_containment_policy",
                key,
                &r.bundle_id,
                &r.policy_id,
                owner,
            )
        }))
    {
        let policy_owner = vocabulary.policies.get(policy);
        if !known_bundles.contains(owner)
            || !keys.insert((kind, key.clone()))
            || endpoint_owner.is_none()
            || policy_owner.is_none()
            || !scope.in_scope(owner, endpoint_owner.as_deref().unwrap_or_default())
            || !scope.in_scope(owner, policy_owner.map(String::as_str).unwrap_or_default())
        {
            diags.push(diagnostic(
                kind,
                format!("attachment '{key}': duplicate, unknown or out-of-closure endpoint/policy"),
                crate::Severity::Fatal,
            ));
        }
    }
    // Missing policy coverage is reported by the privileged runtime doctor.
    // Offline authoring treats warnings as failed edits, so a coverage warning
    // here would block unrelated edits to existing initial-containment recipes.
}

pub fn compile(source: &str, macros: &[DslMacroDefinition]) -> Result<Expression, String> {
    let expression = spellkave_dsl::compile_predicate_with_macros(source, macros)
        .map_err(|failure| format!("{:?}", failure.diagnostics))?;
    for node in &expression.nodes {
        let subject = match &node.op {
            Op::BoolLiteral(_)
            | Op::IntLiteral(_)
            | Op::And(_)
            | Op::Or(_)
            | Op::Not(_)
            | Op::Compare(_)
            | Op::Arithmetic(_)
            | Op::Cond(_) => None,
            Op::ReadInt(read) | Op::ReadBool(read) => Some(&read.subject),
            Op::HasTag(read) => Some(&read.subject),
            Op::HasStatus(read) => Some(&read.subject),
            _ => {
                return Err(format!(
                    "unsupported containment predicate operation at node {}",
                    node.node_id
                ));
            }
        };
        if subject.is_some_and(|subject| !matches!(subject, SubjectRef::Actor | SubjectRef::Target))
        {
            return Err("containment predicates bind only actor and target".into());
        }
    }
    Ok(expression)
}

#[cfg(test)]
mod tests {
    use super::*;

    // keep(K1): shared native/WASM admission rules for the authored predicate boundary.
    #[test]
    fn admits_literal_logic_and_actor_target_state_tags_and_conditions() {
        for source in [
            "true == true",
            "false == true",
            "target.stat('test/open') == 1",
            "actor.has_tag('test/member') && target.status('test/exposed')",
            "target.state('test/open')",
        ] {
            assert!(compile(source, &[]).is_ok(), "{source}");
        }
    }

    // keep(K1): forbidden subject/effect reads must fail even behind a false branch.
    #[test]
    fn rejects_dynamic_stochastic_and_unbound_reads() {
        for source in [
            "actor.source('test/check') > 0",
            "1d20 > 5",
            "world.stat('test/open') == 1",
            "actor.wielded_item.stat('test/open') == 1",
            "actor.has_capability('test/take')",
            "(false == true) && (actor.source('test/check') > 0)",
        ] {
            assert!(compile(source, &[]).is_err(), "accepted {source}");
        }
    }
}

#[cfg(test)]
mod admission_tests {
    use super::*;
    fn manifest() -> Manifest {
        serde_json::from_value(serde_json::json!({
            "schema_version": "1.0.0",
            "bundles": [{"bundle_id":"test", "display_name":"Test", "author":"Test", "version":"1"}],
            "entity_definitions": [{"entity_def_id":"test/body", "bundle_id":"test", "display_name":"Body"}],
            "containment_policy_definition": [{"policy_id":"test/exposed", "bundle_id":"test", "exposure_predicate_source":"true == true", "access_predicate_source":"false == true"}],
            "entity_definition_containment_policy": [{"entity_def_id":"test/body", "bundle_id":"test", "policy_id":"test/exposed"}],
            "entity_definition_initial_containment": [{"entity_def_id":"test/body", "bundle_id":"test", "contained_entity_def_id":"test/item"}]
        })).unwrap()
    }
    fn diagnostics(
        manifest: &Manifest,
        live: Option<&crate::refs::LiveVocabulary>,
    ) -> Vec<crate::Diagnostic> {
        let mut diags = Vec::new();
        validate(manifest, manifest, live, &mut diags);
        diags
    }
    // keep(K1): legacy initial-containment authoring remains editable without a public exposure policy.
    #[test]
    fn attachment_resolves_and_missing_coverage_preserves_authoring() {
        let mut m = manifest();
        assert!(diagnostics(&m, None).is_empty());
        m.entity_definition_containment_policy.clear();
        assert!(diagnostics(&m, None).is_empty());
    }
    // keep(K1): invalid references fail admission before server mutations.
    #[test]
    fn dangling_out_of_scope_and_unbound_inputs_are_rejected() {
        let mut m = manifest();
        m.entity_definition_containment_policy[0].policy_id = "other/unknown".into();
        assert!(diagnostics(&m, None)
            .iter()
            .any(|d| d.severity == crate::Severity::Fatal));
        let mut m = manifest();
        m.containment_policy_definition[0].exposure_predicate_source =
            "target.stat('other/missing') == 1".into();
        assert!(diagnostics(&m, None)
            .iter()
            .any(|d| d.message.contains("out-of-closure input")));
        let mut m = manifest();
        m.containment_policy_definition[0].exposure_predicate_source =
            "actor.source('test/roll') > 0".into();
        assert!(diagnostics(&m, None)
            .iter()
            .any(|d| d.severity == crate::Severity::Fatal));
    }
}
