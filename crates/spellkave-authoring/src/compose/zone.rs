//! Slice 4.5 (#1968 R7, ADR-0087 task 4.5): the `zone` and `env-state`
//! `ActorBehavior` shapes — both lay a lingering `spatial_environment_state`
//! marker on a cell; `zone` also pulses once immediately at cast (a
//! node-local `ForEachActorInRadius`), `env-state` has no pulse at all (a
//! separately-authored consumer affordance reads the marker instead).
//!
//! Ported verbatim from `compose.ts`'s shared `emitZoneMarkerDef`/
//! `zoneMarkerInscribe`/`zonePulseFrameWrite` closures. These three
//! primitives are ALSO used by the `scheduled-zone` shape (task 4.6, not
//! yet ported) — `scheduled-zone`'s own arm keeps calling `compose.ts`'s
//! local closures until its own slice swaps it onto these SAME Rust
//! functions; the closures stay defined in `compose.ts` until then.

use serde::Deserialize;
use serde_json::Value;

use crate::patch::support::{ordered_compact_json, row, s, OrderedField, PatchResult};

/// The subset of `ActorBehavior` fields the zone/env-state primitives read.
#[derive(Deserialize)]
pub struct ZoneBehaviorInput {
    pub shape: String,
    #[serde(rename = "displayName")]
    pub display_name: String,
    #[serde(rename = "zoneStateId", default)]
    pub zone_state_id: Option<String>,
    #[serde(rename = "zoneIntensity", default)]
    pub zone_intensity: Option<Value>,
    #[serde(rename = "zoneVisibility", default)]
    pub zone_visibility: Option<String>,
    #[serde(rename = "envStateValue", default)]
    pub env_state_value: Option<bool>,
    #[serde(rename = "zoneCenterRef", default)]
    pub zone_center_ref: Option<String>,
    #[serde(rename = "predicateSource", default)]
    pub predicate_source: String,
    #[serde(rename = "applyCondition", default)]
    pub apply_condition: Option<String>,
    #[serde(rename = "deltaSource", default)]
    pub delta_source: Option<String>,
}

fn is_env_state(behavior: &ZoneBehaviorInput) -> bool {
    behavior.shape == "env-state"
}

fn zone_center_ref(behavior: &ZoneBehaviorInput) -> String {
    behavior
        .zone_center_ref
        .clone()
        .unwrap_or_else(|| "ActorSpatialRef".to_owned())
}

/// The marker value JSON crosses the wasm boundary as its OWN typed field
/// (`env_state_value: Option<bool>` / `zone_intensity: Option<Value>`, never
/// coerced into each other) — mirrors `compose.ts`'s `markerValue: number |
/// boolean` union, selected by `isEnvState`.
fn marker_value(behavior: &ZoneBehaviorInput) -> Value {
    if is_env_state(behavior) {
        Value::Bool(behavior.env_state_value.unwrap_or(true))
    } else {
        behavior
            .zone_intensity
            .clone()
            .unwrap_or_else(|| Value::from(0))
    }
}

/// The `spatial_state_definitions` marker-def row (`compose.ts`'s
/// `emitZoneMarkerDef`) — the CALLER pushes this onto its own table, exactly
/// like every other cross-cutting row this crate returns rather than
/// mutating in place.
pub fn zone_marker_definition(behavior: &ZoneBehaviorInput, bundle_id: &str) -> Value {
    let env_state = is_env_state(behavior);
    let value_type = if env_state { "bool" } else { "int" };
    let description = if env_state {
        format!(
            "{} ambient env-state marker (bool; consumer reads target.state).",
            behavior.display_name
        )
    } else {
        format!(
            "{} lingering zone marker (value = intensity).",
            behavior.display_name
        )
    };
    row([
        (
            "state_id",
            s(behavior.zone_state_id.clone().unwrap_or_default()),
        ),
        ("bundle_id", s(bundle_id.to_owned())),
        ("value_type", s(value_type)),
        ("writer_policy", s("world_interaction")),
        ("freshness_rule", s("expires_after_ticks:1000000000")),
        ("merge_rule", s("replace")),
        ("lod_policy", s("retain_latest")),
        ("description", s(description)),
    ])
}

/// The marker `Inscribe` frame_write (`compose.ts`'s `zoneMarkerInscribe`) —
/// writes the `spatial_environment_state` marker at RUNTIME on the resolved
/// `zoneCenterRef` cell.
pub fn zone_marker_inscribe(behavior: &ZoneBehaviorInput) -> Value {
    // `payload_json` is a `String`-typed schema field (an OPAQUE
    // `JSON.stringify`'d blob the renderer never re-normalizes) — insertion
    // order must be reproduced byte-for-byte, so this uses
    // `ordered_compact_json`, NOT `row()` + `to_string` (see that helper's
    // doc comment for why the two are not interchangeable here).
    let payload_json = ordered_compact_json(&[
        (
            "state_id",
            OrderedField::Scalar(s(behavior.zone_state_id.clone().unwrap_or_default())),
        ),
        ("value", OrderedField::Scalar(marker_value(behavior))),
        (
            "visibility",
            OrderedField::Scalar(s(behavior
                .zone_visibility
                .clone()
                .unwrap_or_else(|| "Public".to_owned()))),
        ),
    ]);
    row([(
        "Inscribe",
        row([
            ("kind", s("spatial_environment_state")),
            ("location_kind", s("binding")),
            ("location_ref", s(zone_center_ref(behavior))),
            ("payload_json", s(payload_json)),
        ]),
    )])
}

/// The node-local pulse frame_write (`compose.ts`'s `zonePulseFrameWrite`) —
/// a `ForEachActorInRadius(node_ball, ball_radius 0)` wrapping either a
/// damage `ModifyResource` (default) or a save-gated `ApplyStatus` (when
/// `applyCondition` is set). Shared by BOTH `zone`'s immediate pulse and
/// `scheduled-zone`'s deferred re-fired pulse (task 4.6).
pub fn zone_pulse_frame_write(behavior: &ZoneBehaviorInput) -> Value {
    let body = match &behavior.apply_condition {
        Some(condition) if !condition.is_empty() => row([(
            "ApplyStatus",
            row([
                ("target_kind", s("entity")),
                ("target_ref", s("Actor")),
                ("condition_definition_id", s(condition.clone())),
            ]),
        )]),
        _ => row([(
            "ModifyResource",
            row([
                ("actor", s("Actor")),
                ("resource", s("srd/hp")),
                (
                    "delta_source",
                    s(behavior.delta_source.clone().unwrap_or_default()),
                ),
            ]),
        )]),
    };
    row([(
        "ForEachActorInRadius",
        row([
            ("center_ref", s(zone_center_ref(behavior))),
            ("shape_id", s("core/area/node_ball")),
            ("shape_params_json", s(r#"{"ball_radius": 0}"#)),
            ("predicate_source", s(behavior.predicate_source.clone())),
            ("body", Value::Array(vec![body])),
        ]),
    )])
}

/// Parses `behavior_json`/`bundle_id` and returns `zone_marker_definition`
/// as a JSON string.
pub fn zone_marker_definition_json(behavior_json: &str, bundle_id: &str) -> PatchResult<String> {
    let behavior: ZoneBehaviorInput = serde_json::from_str(behavior_json)
        .map_err(|err| format!("zone behavior JSON parse failed: {err}"))?;
    let value = zone_marker_definition(&behavior, bundle_id);
    serde_json::to_string(&value)
        .map_err(|err| format!("zone marker definition output JSON serialization failed: {err}"))
}

/// Parses `behavior_json` and returns `zone_marker_inscribe` as a JSON
/// string.
pub fn zone_marker_inscribe_json(behavior_json: &str) -> PatchResult<String> {
    let behavior: ZoneBehaviorInput = serde_json::from_str(behavior_json)
        .map_err(|err| format!("zone behavior JSON parse failed: {err}"))?;
    let value = zone_marker_inscribe(&behavior);
    serde_json::to_string(&value)
        .map_err(|err| format!("zone marker inscribe output JSON serialization failed: {err}"))
}

/// Parses `behavior_json` and returns `zone_pulse_frame_write` as a JSON
/// string.
pub fn zone_pulse_frame_write_json(behavior_json: &str) -> PatchResult<String> {
    let behavior: ZoneBehaviorInput = serde_json::from_str(behavior_json)
        .map_err(|err| format!("zone behavior JSON parse failed: {err}"))?;
    let value = zone_pulse_frame_write(&behavior);
    serde_json::to_string(&value)
        .map_err(|err| format!("zone pulse frame write output JSON serialization failed: {err}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn env_state_behavior() -> ZoneBehaviorInput {
        serde_json::from_value(json!({
            "shape": "env-state",
            "displayName": "Darkness",
            "zoneStateId": "srd/darkness",
            "predicateSource": "",
        }))
        .unwrap()
    }

    fn zone_behavior() -> ZoneBehaviorInput {
        serde_json::from_value(json!({
            "shape": "zone",
            "displayName": "Spike Growth",
            "zoneStateId": "srd/spike-growth",
            "zoneIntensity": 1,
            "predicateSource": "",
            "deltaSource": "0 - 2d4",
        }))
        .unwrap()
    }

    #[test]
    fn env_state_marker_definition_is_bool_typed() {
        let value = zone_marker_definition(&env_state_behavior(), "srd");
        assert_eq!(value["value_type"], json!("bool"));
        assert_eq!(value["state_id"], json!("srd/darkness"));
    }

    #[test]
    fn zone_marker_definition_is_int_typed() {
        let value = zone_marker_definition(&zone_behavior(), "srd");
        assert_eq!(value["value_type"], json!("int"));
    }

    #[test]
    fn env_state_marker_inscribe_defaults_value_to_true() {
        let value = zone_marker_inscribe(&env_state_behavior());
        let payload_json = value["Inscribe"]["payload_json"].as_str().unwrap();
        let payload: Value = serde_json::from_str(payload_json).unwrap();
        assert_eq!(payload["value"], json!(true));
        assert_eq!(payload["state_id"], json!("srd/darkness"));
        assert_eq!(payload["visibility"], json!("Public"));
    }

    #[test]
    fn zone_marker_inscribe_defaults_value_to_zone_intensity() {
        let value = zone_marker_inscribe(&zone_behavior());
        let payload_json = value["Inscribe"]["payload_json"].as_str().unwrap();
        let payload: Value = serde_json::from_str(payload_json).unwrap();
        assert_eq!(payload["value"], json!(1));
    }

    #[test]
    fn pulse_defaults_to_modify_resource_damage() {
        let value = zone_pulse_frame_write(&zone_behavior());
        assert_eq!(
            value["ForEachActorInRadius"]["body"][0]["ModifyResource"]["delta_source"],
            json!("0 - 2d4")
        );
    }

    #[test]
    fn pulse_uses_apply_status_when_apply_condition_is_set() {
        let mut behavior = zone_behavior();
        behavior.apply_condition = Some("srd/poisoned".to_owned());
        let value = zone_pulse_frame_write(&behavior);
        assert_eq!(
            value["ForEachActorInRadius"]["body"][0]["ApplyStatus"]["condition_definition_id"],
            json!("srd/poisoned")
        );
    }

    #[test]
    fn zone_center_ref_defaults_to_actor_spatial_ref() {
        let value = zone_pulse_frame_write(&zone_behavior());
        assert_eq!(
            value["ForEachActorInRadius"]["center_ref"],
            json!("ActorSpatialRef")
        );
    }

    #[test]
    fn zone_center_ref_target_spatial_ref_override_applies_to_both_marker_and_pulse() {
        let mut behavior = zone_behavior();
        behavior.zone_center_ref = Some("TargetSpatialRef".to_owned());
        assert_eq!(
            zone_marker_inscribe(&behavior)["Inscribe"]["location_ref"],
            json!("TargetSpatialRef")
        );
        assert_eq!(
            zone_pulse_frame_write(&behavior)["ForEachActorInRadius"]["center_ref"],
            json!("TargetSpatialRef")
        );
    }
}
