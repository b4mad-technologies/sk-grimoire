//! Slice 4.2 (#1968 R7, ADR-0087 task 4.2): the `single-target`
//! `ActorBehavior` shape — the simplest topology (one bare frame_write, three
//! body variants). Ported verbatim from `compose.ts`'s single-target `else
//! if` arm: `removeCondition` (the cleanse/restoration family, a
//! `RemoveCondition` frame_write) takes priority, then `applyCondition` (an
//! `ApplyStatus`), else a `ModifyResource` HP/state delta —
//! `resourceStateId` lets the write target a non-HP state (the social
//! skill-check shape), defaulting to `srd/hp`.
//!
//! Only the ONE frame_write body this shape contributes is ported here —
//! this behavior can still be wrapped by the `attacks`/`resourceCost`
//! cross-cutting modifiers (task 4.4, not yet ported), which `compose.ts`
//! applies OUTSIDE this shape's own arm (an `attacks`-carrying behavior never
//! reaches this arm at all — the multiattack check runs first).

use serde::Deserialize;
use serde_json::Value;

use crate::patch::support::{row, s, PatchResult};

/// The subset of `ActorBehavior` fields the single-target shape reads.
#[derive(Deserialize)]
pub struct SingleTargetBehaviorInput {
    #[serde(rename = "removeCondition", default)]
    pub remove_condition: Option<String>,
    #[serde(rename = "removeConditionSource", default)]
    pub remove_condition_source: Option<String>,
    #[serde(rename = "applyCondition", default)]
    pub apply_condition: Option<String>,
    #[serde(rename = "resourceStateId", default)]
    pub resource_state_id: Option<String>,
    #[serde(rename = "deltaSource", default)]
    pub delta_source: Option<String>,
}

/// Non-empty-string truthiness, matching `compose.ts`'s bare `if (b.field)`
/// checks (a JS empty string is falsy, so `field: ""` is treated the same as
/// `field` unset — an author-time no-op, not an error).
fn truthy(value: &Option<String>) -> Option<&str> {
    value.as_deref().filter(|v| !v.is_empty())
}

/// The single-target body: `compose.ts`'s `singleBody` — one frame_write
/// object (`{"RemoveCondition": {...}}` / `{"ApplyStatus": {...}}` /
/// `{"ModifyResource": {...}}`), NOT wrapped in the `frameWrites` array
/// (the caller does that, same as `compose.ts`'s `frameWrites = [singleBody]`).
pub fn single_target_frame_write(behavior: &SingleTargetBehaviorInput) -> Value {
    if let Some(remove_condition) = truthy(&behavior.remove_condition) {
        let source_match = match truthy(&behavior.remove_condition_source) {
            Some(source) => row([("Exact", s(source.to_owned()))]),
            None => s("Any"),
        };
        return row([(
            "RemoveCondition",
            row([
                ("target_kind", s("entity")),
                ("target_ref", s("TargetActor")),
                ("condition_definition_id", s(remove_condition.to_owned())),
                ("source_match", source_match),
            ]),
        )]);
    }
    if let Some(apply_condition) = truthy(&behavior.apply_condition) {
        return row([(
            "ApplyStatus",
            row([
                ("target_kind", s("entity")),
                ("target_ref", s("TargetActor")),
                ("condition_definition_id", s(apply_condition.to_owned())),
            ]),
        )]);
    }
    let resource = truthy(&behavior.resource_state_id)
        .map(str::to_owned)
        .unwrap_or_else(|| "srd/hp".to_owned());
    row([(
        "ModifyResource",
        row([
            ("actor", s("TargetActor")),
            ("resource", s(resource)),
            (
                "delta_source",
                s(behavior.delta_source.clone().unwrap_or_default()),
            ),
        ]),
    )])
}

/// Parses `behavior_json` and returns the single-target frame_write body as
/// a JSON string.
pub fn single_target_frame_write_json(behavior_json: &str) -> PatchResult<String> {
    let behavior: SingleTargetBehaviorInput = serde_json::from_str(behavior_json)
        .map_err(|err| format!("single-target behavior JSON parse failed: {err}"))?;
    let value = single_target_frame_write(&behavior);
    serde_json::to_string(&value)
        .map_err(|err| format!("single-target output JSON serialization failed: {err}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn defaults_to_modify_resource_on_srd_hp() {
        let behavior: SingleTargetBehaviorInput =
            serde_json::from_value(json!({ "deltaSource": "0 - 1d10" })).unwrap();
        let value = single_target_frame_write(&behavior);
        assert_eq!(
            value,
            json!({
                "ModifyResource": {
                    "actor": "TargetActor",
                    "resource": "srd/hp",
                    "delta_source": "0 - 1d10",
                }
            })
        );
    }

    #[test]
    fn resource_state_id_overrides_default_resource() {
        let behavior: SingleTargetBehaviorInput = serde_json::from_value(json!({
            "deltaSource": "d20 + 3",
            "resourceStateId": "srd/attitude",
        }))
        .unwrap();
        let value = single_target_frame_write(&behavior);
        assert_eq!(value["ModifyResource"]["resource"], json!("srd/attitude"));
    }

    #[test]
    fn apply_condition_takes_priority_over_modify_resource() {
        let behavior: SingleTargetBehaviorInput = serde_json::from_value(json!({
            "deltaSource": "0",
            "applyCondition": "srd/paralyzed",
        }))
        .unwrap();
        let value = single_target_frame_write(&behavior);
        assert_eq!(
            value,
            json!({
                "ApplyStatus": {
                    "target_kind": "entity",
                    "target_ref": "TargetActor",
                    "condition_definition_id": "srd/paralyzed",
                }
            })
        );
    }

    #[test]
    fn remove_condition_takes_priority_over_apply_condition() {
        let behavior: SingleTargetBehaviorInput = serde_json::from_value(json!({
            "deltaSource": "0",
            "applyCondition": "srd/paralyzed",
            "removeCondition": "srd/cursed",
            "removeConditionSource": "srd/curse_source",
        }))
        .unwrap();
        let value = single_target_frame_write(&behavior);
        assert_eq!(
            value,
            json!({
                "RemoveCondition": {
                    "target_kind": "entity",
                    "target_ref": "TargetActor",
                    "condition_definition_id": "srd/cursed",
                    "source_match": { "Exact": "srd/curse_source" },
                }
            })
        );
    }

    #[test]
    fn remove_condition_without_source_is_source_agnostic() {
        let behavior: SingleTargetBehaviorInput = serde_json::from_value(json!({
            "deltaSource": "0",
            "removeCondition": "srd/cursed",
        }))
        .unwrap();
        let value = single_target_frame_write(&behavior);
        assert_eq!(value["RemoveCondition"]["source_match"], json!("Any"));
    }

    #[test]
    fn empty_string_condition_fields_are_treated_as_unset() {
        let behavior: SingleTargetBehaviorInput = serde_json::from_value(json!({
            "deltaSource": "0 - 1d6",
            "removeCondition": "",
            "applyCondition": "",
        }))
        .unwrap();
        let value = single_target_frame_write(&behavior);
        assert!(value.get("ModifyResource").is_some());
    }
}
