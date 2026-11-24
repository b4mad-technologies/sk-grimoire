//! Slice 4.7 (#1968 R7, ADR-0087 task 4.7): the `summon` `ActorBehavior`
//! shape (Brief 26 `EffectPrimitive::Spawn`). The cast has NO frame_write at
//! all — the whole effect is the `Spawn` dispatched off the success
//! consequence's `effect_refs_csv`. The spawn location is EITHER a LITERAL
//! authored `place_node` id (`summonLocationNode`) or, when `summonAtCaster`
//! is set, the DYNAMIC `ActorSpatialRef` binding (the caster's current
//! cell — `summonAtCaster` wins if both are present, mirroring
//! `compose.ts`'s own ternary).

use serde::Deserialize;
use serde_json::Value;

use crate::patch::support::{ordered_compact_json, row, s, OrderedField, PatchResult};

/// The subset of `ActorBehavior` fields the summon effect definition reads.
#[derive(Deserialize)]
pub struct SummonBehaviorInput {
    #[serde(rename = "summonBlueprint", default)]
    pub summon_blueprint: Option<String>,
    #[serde(rename = "summonLocationNode", default)]
    pub summon_location_node: Option<String>,
    #[serde(rename = "summonAtCaster", default)]
    pub summon_at_caster: bool,
}

/// The `Spawn` effect_definitions row (`compose.ts`'s `rows.effect_definitions.push({...})`
/// inside the `summon` arm). `spawn_effect_id` is the caller-derived
/// `${action}/spawn` id (from `compose::ids::compose_ids`, computed once by
/// the caller — this function does not re-derive it).
pub fn summon_effect_definition(
    behavior: &SummonBehaviorInput,
    spawn_effect_id: &str,
    bundle_id: &str,
) -> Value {
    // `summonAtCaster: true` → DYNAMIC: `{"kind":"binding","id":"ActorSpatialRef"}`.
    // Otherwise → LITERAL: `{"kind":"place_node","id":<summonLocationNode>}`.
    // Both `kind`/`id` orderings match `compose.ts`'s object literals verbatim.
    let location_ref = ordered_compact_json(&[
        (
            "kind",
            OrderedField::Scalar(s(if behavior.summon_at_caster {
                "binding"
            } else {
                "place_node"
            })),
        ),
        (
            "id",
            OrderedField::Scalar(s(if behavior.summon_at_caster {
                "ActorSpatialRef".to_owned()
            } else {
                behavior.summon_location_node.clone().unwrap_or_default()
            })),
        ),
    ]);
    // `parameters_schema_json` is a `String`-typed schema field (an OPAQUE
    // `JSON.stringify`'d blob) — `ordered_compact_json` with a `Nested`
    // `location_ref` reproduces `compose.ts`'s exact TWO-LEVEL insertion
    // order (blueprint_id, then location_ref's OWN kind/id order), not
    // `row()`'s alphabetical order at either level (see that helper's own
    // doc comment for why).
    let parameters_schema_json = ordered_compact_json(&[
        (
            "blueprint_id",
            OrderedField::Scalar(s(behavior.summon_blueprint.clone().unwrap_or_default())),
        ),
        ("location_ref", OrderedField::Nested(location_ref)),
    ]);
    row([
        ("effect_def_id", s(spawn_effect_id.to_owned())),
        ("bundle_id", s(bundle_id.to_owned())),
        ("primitives_csv", s("Spawn")),
        ("parameters_schema_json", s(parameters_schema_json)),
    ])
}

/// Parses `behavior_json` and returns `summon_effect_definition` as a JSON
/// string.
pub fn summon_effect_definition_json(
    behavior_json: &str,
    spawn_effect_id: &str,
    bundle_id: &str,
) -> PatchResult<String> {
    let behavior: SummonBehaviorInput = serde_json::from_str(behavior_json)
        .map_err(|err| format!("summon behavior JSON parse failed: {err}"))?;
    let value = summon_effect_definition(&behavior, spawn_effect_id, bundle_id);
    serde_json::to_string(&value)
        .map_err(|err| format!("summon effect definition output JSON serialization failed: {err}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn literal_location_node_uses_place_node_kind() {
        let behavior: SummonBehaviorInput = serde_json::from_value(json!({
            "summonBlueprint": "srd/monster/cat",
            "summonLocationNode": "srd_outdoor/cell_a",
        }))
        .unwrap();
        let value = summon_effect_definition(&behavior, "srd/find-familiar/spawn", "srd");
        assert_eq!(value["effect_def_id"], json!("srd/find-familiar/spawn"));
        let params: Value =
            serde_json::from_str(value["parameters_schema_json"].as_str().unwrap()).unwrap();
        assert_eq!(params["blueprint_id"], json!("srd/monster/cat"));
        assert_eq!(params["location_ref"]["kind"], json!("place_node"));
        assert_eq!(params["location_ref"]["id"], json!("srd_outdoor/cell_a"));
    }

    #[test]
    fn summon_at_caster_uses_dynamic_actor_spatial_ref_binding() {
        let behavior: SummonBehaviorInput = serde_json::from_value(json!({
            "summonBlueprint": "srd/monster/imp",
            "summonAtCaster": true,
        }))
        .unwrap();
        let value = summon_effect_definition(&behavior, "srd/conjure-imp/spawn", "srd");
        let params: Value =
            serde_json::from_str(value["parameters_schema_json"].as_str().unwrap()).unwrap();
        assert_eq!(params["location_ref"]["kind"], json!("binding"));
        assert_eq!(params["location_ref"]["id"], json!("ActorSpatialRef"));
    }

    #[test]
    fn summon_at_caster_wins_when_both_are_set() {
        let behavior: SummonBehaviorInput = serde_json::from_value(json!({
            "summonBlueprint": "srd/monster/imp",
            "summonLocationNode": "srd_outdoor/cell_a",
            "summonAtCaster": true,
        }))
        .unwrap();
        let value = summon_effect_definition(&behavior, "srd/conjure-imp/spawn", "srd");
        let params: Value =
            serde_json::from_str(value["parameters_schema_json"].as_str().unwrap()).unwrap();
        assert_eq!(params["location_ref"]["kind"], json!("binding"));
    }

    #[test]
    fn parameters_schema_json_matches_compose_ts_exact_bytes() {
        let behavior: SummonBehaviorInput = serde_json::from_value(json!({
            "summonBlueprint": "srd/monster/cat",
            "summonLocationNode": "srd_outdoor/cell_a",
        }))
        .unwrap();
        let value = summon_effect_definition(&behavior, "srd/find-familiar/spawn", "srd");
        assert_eq!(
            value["parameters_schema_json"],
            json!(
                r#"{"blueprint_id":"srd/monster/cat","location_ref":{"kind":"place_node","id":"srd_outdoor/cell_a"}}"#
            )
        );
    }
}
