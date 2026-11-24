//! Slice 4.6 (#1968 R7, ADR-0087 task 4.6): the `scheduled-zone`
//! `ActorBehavior` shape — the most intricate shape (ADR-0054 autonomy-job
//! scheduling, `zoneCenterRef` anchor/follow semantics). The cast lays the
//! SAME marker `zone`/`env-state` lay (this crate's `compose::zone`
//! primitives, reused verbatim — no re-derivation) and SCHEDULES a
//! subject-bound `ScheduleAutonomyJob` effect that re-fires a deferred pulse
//! (also `compose::zone::zone_pulse_frame_write`, reused) every
//! `scheduleFrequencyMs` for `scheduleRepeats` rounds.
//!
//! This module owns ONLY the piece `zone`/`env-state` don't already provide:
//! the `ScheduleAutonomyJob` effect_definitions row itself.

use serde::Deserialize;
use serde_json::Value;

use crate::patch::support::{ordered_compact_json, row, s, OrderedField, PatchResult};

/// The subset of `ActorBehavior` fields the `ScheduleAutonomyJob` effect
/// definition reads.
#[derive(Deserialize)]
pub struct ScheduledZoneBehaviorInput {
    pub key: String,
    #[serde(rename = "scheduleSource", default)]
    pub schedule_source: Option<String>,
    #[serde(rename = "scheduleFrequencyMs", default)]
    pub schedule_frequency_ms: Option<Value>,
    #[serde(rename = "scheduleRepeats", default)]
    pub schedule_repeats: Option<Value>,
    #[serde(rename = "requiresSubjectAlive", default)]
    pub requires_subject_alive: Option<bool>,
}

/// The `ScheduleAutonomyJob` effect_definitions row (`compose.ts`'s
/// `rows.effect_definitions.push({...})` inside the `scheduled-zone` arm).
/// `action` is the behavior's already-derived action id (from
/// `compose::ids::compose_ids`, computed once per behavior by the caller —
/// this function does not re-derive it) — `effect_def_id` is
/// `${action}/schedule`, the scheduled pulse's `dsl_function` is
/// `${action}/pulse`.
///
/// `scheduleSource`'s default (`b.scheduleSource ?? \`srd/${b.key}\``) is a
/// HARDCODED `"srd/"` literal in `compose.ts` — NOT namespace-aware via
/// `ids()`'s `idNamespace` override. This is preserved VERBATIM (not
/// "fixed") — a bundle-generic caller overriding `idNamespace` must also
/// supply `scheduleSource` explicitly, exactly like `compose.ts`'s existing
/// behavior today.
pub fn schedule_autonomy_job_effect_definition(
    behavior: &ScheduledZoneBehaviorInput,
    action: &str,
    bundle_id: &str,
) -> Value {
    let effect_def_id = format!("{action}/schedule");
    let dsl_function = format!("{action}/pulse");
    let source = behavior
        .schedule_source
        .clone()
        .unwrap_or_else(|| format!("srd/{}", behavior.key));
    // `parameters_schema_json` is a `String`-typed schema field (an OPAQUE
    // `JSON.stringify`'d blob) — `ordered_compact_json` reproduces
    // `compose.ts`'s exact insertion order (dsl_function, fires_at_ms,
    // payload_json, source, frequency_ms, repeats_remaining,
    // requires_subject_alive, priority), NOT `row()`'s alphabetical order
    // (see `ordered_compact_json`'s own doc comment for why the two are not
    // interchangeable for this class of field).
    let parameters_schema_json = ordered_compact_json(&[
        ("dsl_function", OrderedField::Scalar(s(dsl_function))),
        ("fires_at_ms", OrderedField::Scalar(Value::from(6_000))),
        ("payload_json", OrderedField::Scalar(s("{}"))),
        ("source", OrderedField::Scalar(s(source))),
        (
            "frequency_ms",
            OrderedField::Scalar(
                behavior
                    .schedule_frequency_ms
                    .clone()
                    .unwrap_or_else(|| Value::from(6_000)),
            ),
        ),
        (
            "repeats_remaining",
            OrderedField::Scalar(
                behavior
                    .schedule_repeats
                    .clone()
                    .unwrap_or_else(|| Value::from(10)),
            ),
        ),
        (
            "requires_subject_alive",
            OrderedField::Scalar(Value::Bool(
                behavior.requires_subject_alive.unwrap_or(false),
            )),
        ),
        ("priority", OrderedField::Scalar(Value::from(0))),
    ]);
    row([
        ("effect_def_id", s(effect_def_id)),
        ("bundle_id", s(bundle_id.to_owned())),
        ("primitives_csv", s("ScheduleAutonomyJob")),
        ("parameters_schema_json", s(parameters_schema_json)),
    ])
}

/// Parses `behavior_json` and returns `schedule_autonomy_job_effect_definition`
/// as a JSON string.
pub fn schedule_autonomy_job_effect_definition_json(
    behavior_json: &str,
    action: &str,
    bundle_id: &str,
) -> PatchResult<String> {
    let behavior: ScheduledZoneBehaviorInput = serde_json::from_str(behavior_json)
        .map_err(|err| format!("scheduled-zone behavior JSON parse failed: {err}"))?;
    let value = schedule_autonomy_job_effect_definition(&behavior, action, bundle_id);
    serde_json::to_string(&value).map_err(|err| {
        format!("scheduled-zone effect definition output JSON serialization failed: {err}")
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn defaults_behavior() -> ScheduledZoneBehaviorInput {
        serde_json::from_value(json!({ "key": "moonbeam" })).unwrap()
    }

    #[test]
    fn effect_def_id_and_dsl_function_derive_from_action() {
        let value =
            schedule_autonomy_job_effect_definition(&defaults_behavior(), "srd/moonbeam", "srd");
        assert_eq!(value["effect_def_id"], json!("srd/moonbeam/schedule"));
        let params: Value =
            serde_json::from_str(value["parameters_schema_json"].as_str().unwrap()).unwrap();
        assert_eq!(params["dsl_function"], json!("srd/moonbeam/pulse"));
    }

    #[test]
    fn source_defaults_to_hardcoded_srd_prefix_ignoring_namespace() {
        // Preserves compose.ts's exact quirk: the default is `srd/${b.key}`,
        // NOT namespace-aware, even when action carries a different namespace.
        let value = schedule_autonomy_job_effect_definition(
            &defaults_behavior(),
            "aoe_blast/moonbeam",
            "aoe_blast",
        );
        let params: Value =
            serde_json::from_str(value["parameters_schema_json"].as_str().unwrap()).unwrap();
        assert_eq!(params["source"], json!("srd/moonbeam"));
    }

    #[test]
    fn explicit_schedule_source_overrides_the_default() {
        let behavior: ScheduledZoneBehaviorInput = serde_json::from_value(json!({
            "key": "moonbeam",
            "scheduleSource": "custom/source/{Actor}/{tick}",
        }))
        .unwrap();
        let value = schedule_autonomy_job_effect_definition(&behavior, "srd/moonbeam", "srd");
        let params: Value =
            serde_json::from_str(value["parameters_schema_json"].as_str().unwrap()).unwrap();
        assert_eq!(params["source"], json!("custom/source/{Actor}/{tick}"));
    }

    #[test]
    fn defaults_match_compose_ts_literals() {
        let value =
            schedule_autonomy_job_effect_definition(&defaults_behavior(), "srd/moonbeam", "srd");
        let params_json = value["parameters_schema_json"].as_str().unwrap();
        assert_eq!(
            params_json,
            r#"{"dsl_function":"srd/moonbeam/pulse","fires_at_ms":6000,"payload_json":"{}","source":"srd/moonbeam","frequency_ms":6000,"repeats_remaining":10,"requires_subject_alive":false,"priority":0}"#
        );
    }

    #[test]
    fn frequency_repeats_and_subject_alive_overrides_apply() {
        let behavior: ScheduledZoneBehaviorInput = serde_json::from_value(json!({
            "key": "stinking-cloud",
            "scheduleFrequencyMs": 12000,
            "scheduleRepeats": 5,
            "requiresSubjectAlive": true,
        }))
        .unwrap();
        let value = schedule_autonomy_job_effect_definition(&behavior, "srd/stinking-cloud", "srd");
        let params: Value =
            serde_json::from_str(value["parameters_schema_json"].as_str().unwrap()).unwrap();
        assert_eq!(params["frequency_ms"], json!(12_000));
        assert_eq!(params["repeats_remaining"], json!(5));
        assert_eq!(params["requires_subject_alive"], json!(true));
    }
}
