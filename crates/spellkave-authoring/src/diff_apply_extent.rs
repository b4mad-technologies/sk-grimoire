//! Advisory ownership preflight; live placement and final authority stay on the server.
use crate::{
    applyset::PartEntry,
    diff_apply::{ClassifiedRow, RowClass},
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Deserialize)]
pub struct ExtentMemory {
    #[serde(default)]
    pub applyset: String,
    pub bundle_ids: Option<Vec<String>>,
    pub entries: Vec<PartEntry>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ExtentScope {
    pub publisher: bool,
    pub bundle_extents: Vec<String>,
    pub memory: Vec<ExtentMemory>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ExtentVerdict {
    pub kind: String,
    pub key: String,
    pub code: String,
    pub owners: Vec<String>,
}

/// Both sides use the schema's authored storage identity, including bundle-qualified bodies.
pub use spellkave_schema::row_identity::ownership_key;

#[derive(Default)]
struct MemoryReach {
    owners: BTreeSet<String>,
    live: bool,
    satellite: bool,
    target_owners_in_bundle_extents: bool,
    owner_preserving: bool,
    claims_memory: bool,
    retracts: bool,
}

fn memory_refusal(row: &MemoryReach, held: &BTreeSet<String>) -> Option<&'static str> {
    if !row.owners.is_subset(held)
        || (row.satellite
            && (row.claims_memory || row.retracts || !row.owner_preserving)
            && !row.target_owners_in_bundle_extents)
    {
        Some("MF-APPLY-042")
    } else if row.owners.is_empty() && row.live && !row.satellite {
        Some("MF-APPLY-043")
    } else {
        None
    }
}

pub fn memory_verdicts(
    scope: Option<&ExtentScope>,
    applyset: &str,
    root_bundles: &[String],
    rows: &[ClassifiedRow],
    live: &BTreeSet<(String, String)>,
    prune: bool,
) -> Vec<ExtentVerdict> {
    let Some(scope) = scope.filter(|scope| !scope.publisher) else {
        return Vec::new();
    };
    let held: BTreeSet<_> = scope.bundle_extents.iter().cloned().collect();
    let mut ownership = BTreeMap::<_, BTreeSet<String>>::new();
    for memory in &scope.memory {
        let Some(bundles) = &memory.bundle_ids else {
            continue;
        };
        for row in &memory.entries {
            ownership
                .entry(ownership_key(&row.kind, &row.key, row.bundle_id.as_deref()))
                .or_default()
                .extend(bundles.iter().cloned());
        }
    }
    let mut refused = Vec::new();
    for memory in scope.memory.iter().filter(|m| m.applyset == applyset) {
        if memory
            .bundle_ids
            .as_ref()
            .is_none_or(|ids| ids.iter().any(|id| !held.contains(id)))
        {
            refused.push(ExtentVerdict {
                kind: "applyset".into(),
                key: applyset.into(),
                code: "MF-APPLY-042".into(),
                owners: memory.bundle_ids.clone().unwrap_or_default(),
            });
        }
    }
    for bundle in root_bundles.iter().filter(|id| !held.contains(*id)) {
        refused.push(ExtentVerdict {
            kind: "bundles".into(),
            key: bundle.clone(),
            code: "MF-APPLY-042".into(),
            owners: vec![bundle.clone()],
        });
    }
    for row in rows.iter().filter(|row| {
        matches!(
            row.class,
            RowClass::Create | RowClass::Adopt | RowClass::Configure | RowClass::Move
        ) || (prune && row.class == RowClass::Prune)
    }) {
        let key = ownership_key(&row.kind, &row.key, row.bundle_id.as_deref());
        let owners = ownership.get(&key).cloned().unwrap_or_default();
        let code = memory_refusal(
            &MemoryReach {
                owners: owners.clone(),
                live: live.contains(&key),
                satellite: row.kind == "object_state" && owners.is_empty() && live.contains(&key),
                target_owners_in_bundle_extents: row.kind == "object_state"
                    && owners.is_empty()
                    && live.contains(&key)
                    && {
                        let target = serde_json::from_str::<Vec<serde_json::Value>>(&row.key)
                            .ok()
                            .and_then(|values| {
                                values.first().and_then(|v| v.as_str()).map(str::to_owned)
                            });
                        let matches: Vec<_> = live
                            .iter()
                            .filter(|(kind, key)| {
                                kind == "entities"
                                    && target.as_ref().is_some_and(|target| {
                                        if target.contains(':') {
                                            key == target
                                        } else {
                                            key.split_once(':')
                                                .is_some_and(|(_, key)| key == target)
                                        }
                                    })
                            })
                            .collect();
                        matches.len() == 1
                            && ownership
                                .get(matches[0])
                                .is_some_and(|owners| !owners.is_empty() && owners.is_subset(&held))
                    },
                claims_memory: true,
                retracts: row.class == RowClass::Prune,
                ..Default::default()
            },
            &held,
        );
        if let Some(code) = code {
            refused.push(ExtentVerdict {
                kind: row.kind.clone(),
                key: row.key.clone(),
                code: code.into(),
                owners: owners.into_iter().collect(),
            });
        }
    }
    refused
}

#[cfg(test)]
mod tests {
    use super::*;
    fn satellite() -> MemoryReach {
        MemoryReach {
            live: true,
            satellite: true,
            owner_preserving: true,
            target_owners_in_bundle_extents: true,
            ..Default::default()
        }
    }
    #[test]
    fn satellite_memory_policy_preserves_overwrite_and_gates_claims() {
        let held = BTreeSet::from(["house-rules".into()]);
        // Advisory does not resolve placement; raw stamp-preserving state is not an unlisted refusal.
        assert_eq!(memory_refusal(&satellite(), &held), None);
        for row in [
            MemoryReach {
                claims_memory: true,
                ..satellite()
            },
            MemoryReach {
                owner_preserving: false,
                ..satellite()
            },
            MemoryReach {
                retracts: true,
                ..satellite()
            },
        ] {
            assert_eq!(memory_refusal(&row, &held), None);
            let foreign = MemoryReach {
                target_owners_in_bundle_extents: false,
                ..row
            };
            assert_eq!(memory_refusal(&foreign, &held), Some("MF-APPLY-042"));
        }
        let listed = MemoryReach {
            owners: BTreeSet::from(["elven_mini".into()]),
            satellite: false,
            claims_memory: true,
            ..satellite()
        };
        assert_eq!(memory_refusal(&listed, &held), Some("MF-APPLY-042"));
    }
    #[test]
    fn house_rules_hp_claim_uses_hero_memory() {
        let scope = ExtentScope {
            publisher: false,
            bundle_extents: vec!["house-rules".into()],
            memory: vec![ExtentMemory {
                applyset: "hero-root".into(),
                bundle_ids: Some(vec!["elven_mini".into()]),
                entries: vec![PartEntry {
                    export_hash: None,
                    kind: "entities".into(),
                    key: "hero".into(),
                    bundle_id: Some("elven_mini".into()),
                    hash: "h".into(),
                }],
            }],
        };
        let row = ClassifiedRow {
            world_changed: None,
            kind: "object_state".into(),
            key: "[\"hero\",\"srd/hp\"]".into(),
            bundle_id: Some("house-rules".into()),
            ..row("house-rules")
        };
        let live = BTreeSet::from([
            ownership_key(&row.kind, &row.key, row.bundle_id.as_deref()),
            ownership_key("entities", "hero", Some("elven_mini")),
        ]);
        let verdicts = memory_verdicts(
            Some(&scope),
            "house-rules",
            &["house-rules".into()],
            &[row],
            &live,
            false,
        );
        assert_eq!(verdicts.len(), 1);
        assert_eq!(verdicts[0].code, "MF-APPLY-042");
    }
    #[test]
    fn satellite_claim_requires_one_resolved_entity_with_held_owners() {
        let mut scope = ExtentScope {
            publisher: false,
            bundle_extents: vec!["own".into()],
            memory: vec![ExtentMemory {
                applyset: "entity-root".into(),
                bundle_ids: Some(vec!["own".into()]),
                entries: vec![PartEntry {
                    export_hash: None,
                    kind: "entities".into(),
                    key: "hero".into(),
                    bundle_id: Some("own".into()),
                    hash: "h".into(),
                }],
            }],
        };
        let state = ClassifiedRow {
            world_changed: None,
            kind: "object_state".into(),
            key: "[\"hero\",\"srd/hp\"]".into(),
            ..row("own")
        };
        let mut live = BTreeSet::from([
            ownership_key(&state.kind, &state.key, Some("own")),
            ownership_key("entities", "hero", Some("own")),
        ]);
        let check = |scope: &ExtentScope, live: &BTreeSet<(String, String)>| {
            memory_verdicts(
                Some(scope),
                "state-root",
                &["own".into()],
                std::slice::from_ref(&state),
                live,
                false,
            )
        };
        assert!(check(&scope, &live).is_empty());
        // An ambiguous authored reference is not resolved by combining owners.
        live.insert(ownership_key("entities", "hero", Some("second")));
        assert_eq!(check(&scope, &live)[0].code, "MF-APPLY-042");
        scope.memory[0].entries.push(PartEntry {
            export_hash: None,
            kind: "entities".into(),
            key: "hero".into(),
            bundle_id: Some("second".into()),
            hash: "h".into(),
        });
        assert_eq!(check(&scope, &live)[0].code, "MF-APPLY-042");
        scope.memory.clear();
        assert_eq!(check(&scope, &live)[0].code, "MF-APPLY-042");
    }
    fn row(bundle: &str) -> ClassifiedRow {
        ClassifiedRow {
            world_changed: None,
            kind: "entities".into(),
            key: "chest".into(),
            bundle_id: Some(bundle.into()),
            class: RowClass::Configure,
            part: "manifest.toml".into(),
            from_part: None,
            hash: "h".into(),
            live: None,
        }
    }
    #[test]
    fn same_named_chests_have_distinct_memory_and_unknown_rows_are_closed() {
        let scope = ExtentScope {
            publisher: false,
            bundle_extents: vec!["own".into()],
            memory: vec![ExtentMemory {
                applyset: "foreign-root".into(),
                bundle_ids: Some(vec!["foreign".into()]),
                entries: vec![PartEntry {
                    export_hash: None,
                    kind: "entities".into(),
                    key: "chest".into(),
                    bundle_id: Some("foreign".into()),
                    hash: "h".into(),
                }],
            }],
        };
        let live = BTreeSet::from([ownership_key("entities", "chest", Some("own"))]);
        let result = memory_verdicts(
            Some(&scope),
            "own-root",
            &["own".into()],
            &[row("own"), row("foreign")],
            &live,
            false,
        );
        assert_eq!(
            result
                .iter()
                .map(|row| row.code.as_str())
                .collect::<Vec<_>>(),
            ["MF-APPLY-043", "MF-APPLY-042"]
        );
        let publisher = ExtentScope {
            publisher: true,
            ..scope
        };
        assert!(memory_verdicts(
            Some(&publisher),
            "foreign-root",
            &["foreign".into()],
            &[row("foreign")],
            &live,
            true
        )
        .is_empty());
    }
    #[test]
    fn an_empty_delta_cannot_adopt_a_foreign_or_legacy_root() {
        for ids in [None, Some(vec!["foreign".into()])] {
            let scope = ExtentScope {
                publisher: false,
                bundle_extents: vec!["own".into()],
                memory: vec![ExtentMemory {
                    applyset: "root".into(),
                    bundle_ids: ids,
                    entries: Vec::new(),
                }],
            };
            assert_eq!(
                memory_verdicts(
                    Some(&scope),
                    "root",
                    &["own".into()],
                    &[],
                    &BTreeSet::new(),
                    false
                )[0]
                .code,
                "MF-APPLY-042"
            );
        }
    }
}
