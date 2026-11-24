//! Authored condition-derived extents; no persistent runtime grant is created.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// While an admitted condition is active on a body, derive this capability
/// and extent at the Hidden disclosure floor. Stored grant ceilings are unchanged.
/// The four reference fields form the storage key; bundle_id owns the rule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ConditionCapabilityExtentGrantRow {
    pub condition_definition_id: String,
    pub capability_id: String,
    pub extent_kind_ref: String,
    pub extent_ref: String,
    pub bundle_id: String,
}
