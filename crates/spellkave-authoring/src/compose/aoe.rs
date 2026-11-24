//! Slice 4.3 (#1968 R7, ADR-0087 task 4.3): the `aoe` `ActorBehavior`
//! shape — a `ForEachActorInRadius` sphere (or an overridden topological
//! shape), body-gated OR delta-gated per ADR-0045's two save seams.
//!
//! This is the shape `authoring-verbs.ts`'s `shapeParamsJson` hand-mirrored
//! (the incident that motivated this whole migration — ADR-0087 Context):
//! `compose.ts`'s own default formatting for `areaShapeParamsJson`
//! (`{"radius_m": 5.0}`, a space after the colon, `toFixed(1)`) was
//! independently re-derived by `authoring-verbs.ts` instead of reused, and
//! the two copies drifted (`JSON.stringify`'s compact form vs. the literal
//! spacing). `shape_params_json` below is now the ONE place either TS caller
//! computes this formatting — `compose.ts`'s aoe default and
//! `authoring-verbs.ts`'s `shapeParamsJson` both call it.

use serde::Deserialize;
use serde_json::Value;

use crate::patch::support::{row, s, PatchResult};

/// The subset of `ActorBehavior` fields the aoe shape reads.
#[derive(Deserialize)]
pub struct AoeBehaviorInput {
    #[serde(rename = "areaShapeId", default)]
    pub area_shape_id: Option<String>,
    #[serde(rename = "areaShapeParamsJson", default)]
    pub area_shape_params_json: Option<String>,
    #[serde(rename = "radiusM", default)]
    pub radius_m: Option<Value>,
    #[serde(rename = "predicateSource")]
    pub predicate_source: String,
    #[serde(rename = "aoeResource", default)]
    pub aoe_resource: Option<String>,
    #[serde(rename = "deltaSource", default)]
    pub delta_source: Option<String>,
}

/// `authoring-verbs.ts`'s `shapeParamsJson(shapeId, radius)`, minus its own
/// tool-specific error text (host ergonomics — the `DEFINE_BEHAVIOR_TOOL`/
/// `SHAPE_ID_VALUES` wording stays in TS, which pre-validates `shapeId`
/// before calling this; an unrecognized id here is a defensive fallback, not
/// the caller-facing error path). `"core/area/node_ball"` formats
/// `{"ball_radius": <radius>}` (plain JS `${radius}` coercion — no forced
/// decimal); `"core/area/sphere"` formats `{"radius_m": <radius>}` with
/// `toFixed(1)` (always exactly one decimal digit, e.g. `5.0`).
pub fn shape_params_json(shape_id: &str, radius: &Value) -> PatchResult<String> {
    match shape_id {
        "core/area/node_ball" => Ok(format!("{{\"ball_radius\": {}}}", plain_number(radius))),
        "core/area/sphere" => Ok(format!("{{\"radius_m\": {}}}", to_fixed_1(radius))),
        other => Err(format!("unsupported area shape id: {other}")),
    }
}

/// `${radius}` — plain JS Number-to-string coercion. For the integer-valued
/// radii every real caller passes (a topological `ball_radius`), the
/// original JSON token's own Display already matches JS's coercion exactly;
/// carrying it through avoids ANY f64 round-trip.
fn plain_number(radius: &Value) -> String {
    match radius {
        Value::Number(n) => n.to_string(),
        other => other.to_string(),
    }
}

/// `radius.toFixed(1)` — always exactly one decimal digit. This is a real
/// numeric transform (JS's `toFixed` rounds via the double value), not a
/// token passthrough, so converting through `f64` here is correct — unlike
/// fields this crate carries untouched (see `patch::support`'s module doc),
/// `toFixed(1)` is DESIGNED to always emit a decimal point.
fn to_fixed_1(radius: &Value) -> String {
    let value = radius.as_f64().unwrap_or(0.0);
    format!("{value:.1}")
}

/// `compose.ts`'s aoe (default) shape's one frame_write:
/// `{"ForEachActorInRadius": {...}}`. When `areaShapeParamsJson` is unset,
/// the default is computed via `shape_params_json` — ALWAYS against
/// `"core/area/sphere"`, regardless of `areaShapeId` (a caller overriding
/// the shape id must also supply `areaShapeParamsJson` explicitly; this
/// mirrors `compose.ts`'s own historical behavior verbatim — the default
/// formula never branches on `areaShapeId`).
pub fn aoe_frame_write(behavior: &AoeBehaviorInput) -> PatchResult<Value> {
    let shape_id = behavior
        .area_shape_id
        .clone()
        .unwrap_or_else(|| "core/area/sphere".to_owned());
    let shape_params_json = match &behavior.area_shape_params_json {
        Some(params) => params.clone(),
        None => {
            let radius = behavior.radius_m.clone().unwrap_or_else(|| Value::from(0));
            shape_params_json("core/area/sphere", &radius)?
        }
    };
    let resource = behavior
        .aoe_resource
        .clone()
        .unwrap_or_else(|| "srd/hp".to_owned());
    Ok(row([(
        "ForEachActorInRadius",
        row([
            ("center_ref", s("ActorSpatialRef")),
            ("shape_id", s(shape_id)),
            ("shape_params_json", s(shape_params_json)),
            ("predicate_source", s(behavior.predicate_source.clone())),
            (
                "body",
                Value::Array(vec![row([(
                    "ModifyResource",
                    row([
                        ("actor", s("Actor")),
                        ("resource", s(resource)),
                        (
                            "delta_source",
                            s(behavior.delta_source.clone().unwrap_or_default()),
                        ),
                    ]),
                )])]),
            ),
        ]),
    )]))
}

/// Parses `behavior_json` and returns the aoe frame_write as a JSON string.
pub fn aoe_frame_write_json(behavior_json: &str) -> PatchResult<String> {
    let behavior: AoeBehaviorInput = serde_json::from_str(behavior_json)
        .map_err(|err| format!("aoe behavior JSON parse failed: {err}"))?;
    let value = aoe_frame_write(&behavior)?;
    serde_json::to_string(&value)
        .map_err(|err| format!("aoe output JSON serialization failed: {err}"))
}

/// Parses `radius_json` (a bare JSON number) and returns
/// `shape_params_json(shape_id, radius)` as a JSON string. The wasm-facing
/// entry point `authoring-verbs.ts`'s `shapeParamsJson` calls directly.
pub fn shape_params_json_call(shape_id: &str, radius_json: &str) -> PatchResult<String> {
    let radius: Value = serde_json::from_str(radius_json)
        .map_err(|err| format!("shape params radius JSON parse failed: {err}"))?;
    shape_params_json(shape_id, &radius)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn sphere_default_matches_the_historical_to_fixed_1_literal() {
        assert_eq!(
            shape_params_json("core/area/sphere", &json!(5)).unwrap(),
            r#"{"radius_m": 5.0}"#
        );
        assert_eq!(
            shape_params_json("core/area/sphere", &json!(7.5)).unwrap(),
            r#"{"radius_m": 7.5}"#
        );
    }

    #[test]
    fn node_ball_uses_plain_coercion_no_forced_decimal() {
        assert_eq!(
            shape_params_json("core/area/node_ball", &json!(0)).unwrap(),
            r#"{"ball_radius": 0}"#
        );
    }

    #[test]
    fn unsupported_shape_id_errors() {
        let err = shape_params_json("core/area/cube", &json!(1)).unwrap_err();
        assert!(err.contains("core/area/cube"));
    }

    #[test]
    fn aoe_default_frame_write_uses_sphere_formula_regardless_of_area_shape_id_override() {
        // Mirrors compose.ts's own quirk: leaving areaShapeParamsJson unset
        // ALWAYS computes the sphere formula, even if areaShapeId names a
        // different shape — a caller overriding the shape id must also supply
        // areaShapeParamsJson explicitly.
        let behavior: AoeBehaviorInput = serde_json::from_value(json!({
            "areaShapeId": "core/area/node_ball",
            "radiusM": 10,
            "predicateSource": "d20 >= 13",
            "deltaSource": "0 - 4d6",
        }))
        .unwrap();
        let value = aoe_frame_write(&behavior).unwrap();
        assert_eq!(
            value["ForEachActorInRadius"]["shape_params_json"],
            json!(r#"{"radius_m": 10.0}"#)
        );
    }

    #[test]
    fn explicit_area_shape_params_json_override_bypasses_computation() {
        let behavior: AoeBehaviorInput = serde_json::from_value(json!({
            "areaShapeId": "core/area/node_ball",
            "areaShapeParamsJson": r#"{"ball_radius": 0}"#,
            "aoeResource": "aoe_blast/hp",
            "predicateSource": "d20 < 13",
            "deltaSource": "-d6",
        }))
        .unwrap();
        let value = aoe_frame_write(&behavior).unwrap();
        assert_eq!(
            value,
            json!({
                "ForEachActorInRadius": {
                    "center_ref": "ActorSpatialRef",
                    "shape_id": "core/area/node_ball",
                    "shape_params_json": r#"{"ball_radius": 0}"#,
                    "predicate_source": "d20 < 13",
                    "body": [
                        { "ModifyResource": { "actor": "Actor", "resource": "aoe_blast/hp", "delta_source": "-d6" } }
                    ],
                }
            })
        );
    }

    #[test]
    fn default_shape_id_and_resource_match_original_sphere_srd_hp_behavior() {
        let behavior: AoeBehaviorInput = serde_json::from_value(json!({
            "radiusM": 20,
            "predicateSource": "",
            "deltaSource": "cond(d20 >= 13, 0 - (8d6 / 2), 0 - 8d6)",
        }))
        .unwrap();
        let value = aoe_frame_write(&behavior).unwrap();
        assert_eq!(
            value["ForEachActorInRadius"]["shape_id"],
            json!("core/area/sphere")
        );
        assert_eq!(
            value["ForEachActorInRadius"]["body"][0]["ModifyResource"]["resource"],
            json!("srd/hp")
        );
    }
}
