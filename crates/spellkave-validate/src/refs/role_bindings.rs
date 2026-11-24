//! Authored binding references are checked without realizing authority.
use super::{diag_fatal, diag_warning, BundleScope, LiveVocabulary};
use crate::{Diagnostic, MF_VAL_FK_CLOSURE};
use spellkave_schema::{Manifest, PLAYER_SEAT_TAG};
use std::collections::BTreeMap;

pub const MF_VAL_BINDING_ROLE_REF: &str = "MF-VAL-369";
pub const MF_VAL_BINDING_SUBJECT: &str = "MF-VAL-370";
pub const MF_VAL_BINDING_PRIVILEGED_ROLE: &str = "MF-VAL-371";
pub const MF_VAL_PRESENCE_IDENTITY_LABEL: &str = "MF-VAL-372";

/// Index only subject provenance and playability; do not revalidate live actors.
pub fn authored_subjects(manifest: &Manifest) -> BTreeMap<String, (String, bool)> {
    authored_subjects_with_live(manifest, None)
}

fn authored_subjects_with_live(
    manifest: &Manifest,
    live: Option<&LiveVocabulary>,
) -> BTreeMap<String, (String, bool)> {
    let rows = manifest
        .actors
        .iter()
        .map(|actor| {
            let playable_instance = manifest
                .entity_tag
                .iter()
                .any(|tag| tag.entity_key == actor.entity_key && tag.tag == PLAYER_SEAT_TAG);
            let playable_definition = manifest
                .entities
                .iter()
                .filter(|entity| entity.entity_key == actor.entity_key)
                .any(|entity| {
                    live.is_some_and(|world| {
                        world
                            .playable_entity_definitions
                            .contains(&entity.entity_def_id)
                    }) || manifest.entity_definition_tag.iter().any(|tag| {
                        tag.entity_def_id == entity.entity_def_id && tag.tag == PLAYER_SEAT_TAG
                    })
                });
            (
                actor.entity_key.clone(),
                (
                    actor.bundle_id.clone(),
                    playable_instance || playable_definition,
                ),
            )
        })
        .collect::<Vec<_>>();
    let mut subjects = BTreeMap::new();
    merge_subjects(&mut subjects, rows);
    subjects
}

pub fn merge_subjects(
    subjects: &mut BTreeMap<String, (String, bool)>,
    rows: impl IntoIterator<Item = (String, (String, bool))>,
) {
    for (key, subject) in rows {
        match subjects.get_mut(&key) {
            Some(old) if old.0 != subject.0 => *old = (String::new(), true),
            // A partial declaration does not prove removal of a live seat tag.
            Some(old) => old.1 |= subject.1,
            None => {
                subjects.insert(key, subject);
            }
        }
    }
}

fn identity_subject(subject: &str) -> bool {
    let hex = subject.strip_prefix("0x").unwrap_or(subject);
    hex.len() == 64 && hex.bytes().all(|byte| byte.is_ascii_hexdigit())
}

pub(super) fn validate(
    manifest: &Manifest,
    live: Option<&LiveVocabulary>,
    scope: &BundleScope,
    diags: &mut Vec<Diagnostic>,
) {
    let bootstrap = if live.is_none() {
        spellkave_format::parse_manifest(spellkave_schema::CORE_BOOTSTRAP_MANIFEST)
            .expect("schema bootstrap parses")
            .manifest
    } else {
        Manifest::default()
    };
    let mut roles: BTreeMap<_, _> = bootstrap
        .role_definitions
        .iter()
        .map(|row| (row.role_id.clone(), row.clone()))
        .collect();
    let mut aggregations = bootstrap.role_aggregations.clone();
    let mut subjects = BTreeMap::new();
    let mut graphs = BTreeMap::new();
    if let Some(live) = live {
        roles.extend(live.roles.clone());
        aggregations.extend(live.role_aggregations.clone());
        subjects.extend(live.role_subjects.clone());
        graphs.extend(live.place_graphs.clone());
    }
    roles.extend(
        manifest
            .role_definitions
            .iter()
            .map(|row| (row.role_id.clone(), row.clone())),
    );
    for row in &manifest.role_aggregations {
        aggregations.retain(|old| old.role_id != row.role_id || old.bundle_id != row.bundle_id);
        aggregations.push(row.clone());
    }
    merge_subjects(&mut subjects, authored_subjects_with_live(manifest, live));
    graphs.extend(
        manifest
            .place_graphs
            .iter()
            .map(|row| (row.place_graph_id.clone(), row.bundle_id.clone())),
    );
    let definitions: Vec<_> = roles.values().cloned().collect();
    let mut extent_kinds: BTreeMap<_, _> = spellkave_schema::CORE_EXTENT_KINDS
        .iter()
        .map(|row| (row.kind_id, row.resolution_shape))
        .collect();
    if let Some(world) = live {
        extent_kinds.extend(
            world
                .extent_shapes
                .iter()
                .map(|(id, shape)| (id.as_str(), shape.as_str())),
        );
    }
    extent_kinds.extend(
        manifest
            .extent_kind_definitions
            .iter()
            .map(|row| (row.kind_id.as_str(), row.resolution_shape.as_str())),
    );
    let mut preconditions = std::collections::BTreeSet::new();
    for precondition in &manifest.binding_preconditions {
        if !valid_precondition(precondition)
            || !preconditions.insert(&precondition.binding_id)
            || !manifest
                .role_bindings
                .iter()
                .any(|row| row.binding_id == precondition.binding_id)
        {
            diags.push(diag_fatal(
                crate::diagnostic_codes::MF_VAL_ROLE_BINDING_CONSTRAINT,
                "binding_preconditions",
                "invalid, duplicate or unmatched binding precondition".into(),
            ));
        }
    }
    for row in &manifest.role_bindings {
        if row
            .handover_note
            .as_ref()
            .is_some_and(|note| note.len() > 16 * 1024)
        {
            diags.push(diag_fatal(
                crate::diagnostic_codes::MF_VAL_ROLE_BINDING_CONSTRAINT,
                "role_bindings.handover_note",
                "shared briefing exceeds 16384 UTF-8 bytes".into(),
            ));
        }
        let role = roles.get(&row.role_id).filter(|role| {
            (live.is_some() && row.bundle_id.is_empty())
                || scope.in_scope(&row.bundle_id, &role.bundle_id)
        });
        if role.is_none() {
            diags.push(diag_fatal(
                MF_VAL_BINDING_ROLE_REF,
                "role_bindings.role_id",
                format!(
                    "binding '{}' role '{}' does not resolve in bundle '{}' scope",
                    row.binding_id, row.role_id, row.bundle_id
                ),
            ));
        } else if role.is_some_and(|role| role.bundle_id != "core")
            && super::roles::merged_role_capability_ids(&row.role_id, &definitions, &aggregations)
                .iter()
                .any(|id| id.starts_with(super::privileged_namespace::PRIVILEGED_CAPABILITY_PREFIX))
        {
            diags.push(diag_warning(MF_VAL_BINDING_PRIVILEGED_ROLE, "role_bindings.role_id",
                format!("binding '{}' uses a third-party privileged role: live admission needs publisher/genesis, a covering bind right or every merged id; a role edit also checks every affected realised binding's old and new set", row.binding_id)));
        }
        if row.subject != "$sender"
            && !identity_subject(&row.subject)
            && !subjects.get(&row.subject).is_some_and(|(owner, playable)| {
                !playable
                    && ((live.is_some() && row.bundle_id.is_empty())
                        || scope.in_scope(&row.bundle_id, owner))
            })
        {
            diags.push(diag_fatal(MF_VAL_BINDING_SUBJECT, "role_bindings.subject",
                format!("binding '{}' requires $sender, a hexadecimal identity, or a non-playable authored actor in scope", row.binding_id)));
        }
        let kind = if row.extent_kind_ref.is_empty() {
            "place_graph"
        } else {
            &row.extent_kind_ref
        };
        match extent_kinds.get(kind) {
            None => diags.push(diag_fatal(
                MF_VAL_FK_CLOSURE,
                "role_bindings.extent_kind_ref",
                format!(
                    "binding '{}' references unknown extent kind '{kind}'",
                    row.binding_id
                ),
            )),
            Some(&"PlaceGraph")
                if !row.extent_ref.is_empty() && !graphs.contains_key(&row.extent_ref) =>
            {
                diags.push(diag_fatal(
                    MF_VAL_FK_CLOSURE,
                    "role_bindings.extent_ref",
                    format!(
                        "binding '{}' references unknown place graph '{}'",
                        row.binding_id, row.extent_ref
                    ),
                ))
            }
            _ => {}
        }
    }
    for row in &manifest.actor_identities {
        if !matches!(
            row.provenance.as_deref(),
            None | Some("person") | Some("service")
        ) {
            diags.push(diag_fatal(
                crate::diagnostic_codes::MF_VAL_IDENTITY_PROVENANCE,
                "actor_identities.provenance",
                "identity provenance must be person or service".into(),
            ));
        }
        if row.role == "co-pilot"
            && manifest.actor_identities.iter().any(|binding| {
                binding.bundle_id == row.bundle_id
                    && binding.entity_key == row.entity_key
                    && binding.provenance.as_deref() == Some("person")
            })
        {
            diags.push(diag_fatal(crate::diagnostic_codes::MF_VAL_AUTHORED_CO_PILOT, "actor_identities.role", "authored co-pilot requires a durable Service primary and no Person row; the server checks live-only bindings".into()));
        }
        if row.role == "presence" {
            diags.push(diag_fatal(
                MF_VAL_PRESENCE_IDENTITY_LABEL,
                "actor_identities.role",
                "the presence identity label is reserved for realized role bindings".into(),
            ));
        }
    }
}

fn valid_precondition(p: &spellkave_schema::BindingPrecondition) -> bool {
    let digest = |s: &str| {
        s.len() == 64
            && s.bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
    };
    !p.binding_id.is_empty()
        && digest(&p.expected_digest)
        && digest(&p.expected_merged_role_digest)
        && !p.expected_incarnation.is_empty()
        && p.expected_incarnation.len() <= 256
        && uuid_shape(&p.request_id)
        && !p.reason.trim().is_empty()
        && p.reason.len() <= 1024
}
fn uuid_shape(s: &str) -> bool {
    s.len() == 36
        && s.bytes().enumerate().all(|(i, b)| {
            if [8, 13, 18, 23].contains(&i) {
                b == b'-'
            } else {
                b.is_ascii_hexdigit()
            }
        })
}
#[cfg(test)]
mod handover_tests {
    use super::*;
    #[test]
    fn precondition_digest_and_reason_bounds() {
        let mut p = spellkave_schema::BindingPrecondition {
            binding_id: "binding".into(),
            expected_digest: "a".repeat(64),
            expected_merged_role_digest: "b".repeat(64),
            expected_incarnation: "epoch/1".into(),
            expected_revision: 4,
            request_id: "12345678-1234-1234-1234-123456789abc".into(),
            reason: "é".repeat(512),
        };
        assert!(valid_precondition(&p));
        p.reason.push('é');
        assert!(!valid_precondition(&p));
        p.reason = "handover".into();
        p.expected_digest = "g".repeat(64);
        assert!(!valid_precondition(&p));
        p.expected_digest = "a".repeat(64);
        p.request_id = "invalid".into();
        assert!(!valid_precondition(&p));
    }
}
