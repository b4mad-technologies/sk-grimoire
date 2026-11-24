//! Authored containment exposure and removal policy attachments.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ContainmentPolicyDefinitionRow {
    pub policy_id: String,
    pub bundle_id: String,
    pub exposure_predicate_source: String,
    pub access_predicate_source: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EntityDefinitionContainmentPolicyRow {
    pub entity_def_id: String,
    pub policy_id: String,
    pub bundle_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EntityContainmentPolicyRow {
    pub entity_key: String,
    pub policy_id: String,
    pub bundle_id: String,
}
