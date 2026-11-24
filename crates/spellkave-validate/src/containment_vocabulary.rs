//! Existence and ownership facts shared by containment validation and live admission.

use std::collections::BTreeMap;

use spellkave_schema::Manifest;

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

pub(crate) fn qualified(bundle: &str, key: &str) -> String {
    if key.contains(':') {
        key.into()
    } else {
        format!("{bundle}:{key}")
    }
}
