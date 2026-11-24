//! Slice 4.9 (#1968 R7, ADR-0087 task 4.9): the `clock` `ActorBehavior`
//! shape (#1968 finding C2, the death-saves shape). A SELF-TARGETED
//! multi-ordinal tally/threshold pattern — every ordinal targets `"Actor"`
//! ALONE (never `TargetActor`, never a bare top-level `ApplyStatus`). Tally
//! ordinals come FIRST (array order = ordinal order), threshold ordinals
//! AFTER — each threshold's `when` reads the tallies' just-written values
//! (ordinal-sorted sequential commit).

use serde::Deserialize;
use serde_json::Value;

use crate::patch::support::{row, s, PatchResult};

/// One `ClockTally` entry — `compose.ts`'s `ActorBehavior.clockTallies[]`.
#[derive(Deserialize)]
pub struct ClockTallyInput {
    pub resource: String,
    #[serde(default)]
    pub predicate: Option<String>,
    #[serde(default)]
    pub delta: Option<String>,
}

/// One `ClockThreshold` entry — `compose.ts`'s `ActorBehavior.clockThresholds[]`.
#[derive(Deserialize)]
pub struct ClockThresholdInput {
    pub when: String,
    #[serde(rename = "applyStatus", default)]
    pub apply_status: Option<String>,
    #[serde(rename = "removeCondition", default)]
    pub remove_condition: Option<String>,
    #[serde(rename = "removeConditionSource", default)]
    pub remove_condition_source: Option<String>,
}

/// One tally ordinal's `ModifyResource` frame_write — `compose.ts`'s
/// `tallyWrites` map. `predicate` set → increment-on-condition
/// (`cond(<predicate>, <delta ?? "1">, 0)`, the tally-split idiom).
/// `predicate` unset → `delta` is the whole raw expression verbatim (the
/// scratch-cell capture idiom, e.g. `"d20 - actor.stat('<scratch>')"` — not
/// a conditional at all).
fn tally_frame_write(tally: &ClockTallyInput) -> Value {
    let delta_source = match &tally.predicate {
        Some(predicate) => format!(
            "cond({predicate}, {}, 0)",
            tally.delta.as_deref().unwrap_or("1")
        ),
        None => tally.delta.clone().unwrap_or_else(|| "0".to_owned()),
    };
    row([(
        "ModifyResource",
        row([
            ("actor", s("Actor")),
            ("resource", s(tally.resource.clone())),
            ("delta_source", s(delta_source)),
        ]),
    )])
}

/// One threshold ordinal's `ForEachActorInRadius(node_ball, ball_radius 0)`
/// frame_write wrapping ONE `ApplyStatus`/`RemoveCondition` on `"Actor"` —
/// `compose.ts`'s `thresholdWrites` map. `applyStatus` set (even to an empty
/// string — the check is `!== undefined`, not truthiness) wins over
/// `removeCondition`.
fn threshold_frame_write(threshold: &ClockThresholdInput) -> Value {
    let body = match &threshold.apply_status {
        Some(apply_status) => row([(
            "ApplyStatus",
            row([
                ("target_kind", s("entity")),
                ("target_ref", s("Actor")),
                ("condition_definition_id", s(apply_status.clone())),
            ]),
        )]),
        None => {
            let source_match = match threshold
                .remove_condition_source
                .as_deref()
                .filter(|v| !v.is_empty())
            {
                Some(source) => row([("Exact", s(source.to_owned()))]),
                None => s("Any"),
            };
            row([(
                "RemoveCondition",
                row([
                    ("target_kind", s("entity")),
                    ("target_ref", s("Actor")),
                    (
                        "condition_definition_id",
                        s(threshold.remove_condition.clone().unwrap_or_default()),
                    ),
                    ("source_match", source_match),
                ]),
            )])
        }
    };
    row([(
        "ForEachActorInRadius",
        row([
            ("center_ref", s("ActorSpatialRef")),
            ("shape_id", s("core/area/node_ball")),
            ("shape_params_json", s(r#"{"ball_radius": 0}"#)),
            ("predicate_source", s(threshold.when.clone())),
            ("body", Value::Array(vec![body])),
        ]),
    )])
}

/// The clock shape's full `frameWrites` array — every tally ordinal (array
/// order preserved), THEN every threshold ordinal (array order preserved).
/// Mirrors `compose.ts`'s `frameWrites = [...tallyWrites, ...thresholdWrites]`.
pub fn clock_frame_writes(
    tallies: &[ClockTallyInput],
    thresholds: &[ClockThresholdInput],
) -> Vec<Value> {
    tallies
        .iter()
        .map(tally_frame_write)
        .chain(thresholds.iter().map(threshold_frame_write))
        .collect()
}

/// The subset of `ActorBehavior` fields the clock shape reads.
#[derive(Deserialize)]
pub struct ClockBehaviorInput {
    #[serde(rename = "clockTallies", default)]
    pub clock_tallies: Option<Vec<ClockTallyInput>>,
    #[serde(rename = "clockThresholds", default)]
    pub clock_thresholds: Option<Vec<ClockThresholdInput>>,
}

/// Parses `behavior_json` and returns `clock_frame_writes` as a JSON string
/// (a JSON array).
pub fn clock_frame_writes_json(behavior_json: &str) -> PatchResult<String> {
    let behavior: ClockBehaviorInput = serde_json::from_str(behavior_json)
        .map_err(|err| format!("clock behavior JSON parse failed: {err}"))?;
    let tallies = behavior.clock_tallies.unwrap_or_default();
    let thresholds = behavior.clock_thresholds.unwrap_or_default();
    let value = Value::Array(clock_frame_writes(&tallies, &thresholds));
    serde_json::to_string(&value)
        .map_err(|err| format!("clock output JSON serialization failed: {err}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn death_saves_tallies() -> Vec<ClockTallyInput> {
        serde_json::from_value(json!([
            {
                "resource": "srd/death_save_last_roll",
                "delta": "d20 - actor.stat('srd/death_save_last_roll')",
            },
            {
                "resource": "srd/death_save_successes",
                "predicate": "actor.stat('srd/death_save_last_roll') >= 10",
            },
            {
                "resource": "srd/death_save_failures",
                "predicate": "actor.stat('srd/death_save_last_roll') < 10",
            },
        ]))
        .unwrap()
    }

    fn death_saves_thresholds() -> Vec<ClockThresholdInput> {
        serde_json::from_value(json!([
            { "when": "actor.stat('srd/death_save_successes') >= 3", "applyStatus": "srd/stable" },
            { "when": "actor.stat('srd/death_save_failures') >= 3", "applyStatus": "srd/dead" },
        ]))
        .unwrap()
    }

    #[test]
    fn scratch_capture_tally_without_predicate_uses_delta_verbatim() {
        let tallies = death_saves_tallies();
        let value = tally_frame_write(&tallies[0]);
        assert_eq!(
            value["ModifyResource"]["delta_source"],
            json!("d20 - actor.stat('srd/death_save_last_roll')")
        );
    }

    #[test]
    fn predicate_tally_folds_to_cond_with_default_delta_one() {
        let tallies = death_saves_tallies();
        let value = tally_frame_write(&tallies[1]);
        assert_eq!(
            value["ModifyResource"]["delta_source"],
            json!("cond(actor.stat('srd/death_save_last_roll') >= 10, 1, 0)")
        );
    }

    #[test]
    fn predicate_tally_with_explicit_delta_overrides_the_default_one() {
        let tally: ClockTallyInput = serde_json::from_value(json!({
            "resource": "srd/curse_stacks",
            "predicate": "d20 >= 15",
            "delta": "2",
        }))
        .unwrap();
        let value = tally_frame_write(&tally);
        assert_eq!(
            value["ModifyResource"]["delta_source"],
            json!("cond(d20 >= 15, 2, 0)")
        );
    }

    #[test]
    fn threshold_apply_status_targets_actor_never_target_actor() {
        let thresholds = death_saves_thresholds();
        let value = threshold_frame_write(&thresholds[0]);
        assert_eq!(
            value["ForEachActorInRadius"]["body"][0]["ApplyStatus"]["target_ref"],
            json!("Actor")
        );
        assert_eq!(
            value["ForEachActorInRadius"]["center_ref"],
            json!("ActorSpatialRef")
        );
    }

    #[test]
    fn threshold_remove_condition_defaults_to_source_agnostic_any() {
        let threshold: ClockThresholdInput = serde_json::from_value(json!({
            "when": "actor.stat('srd/curse_stacks') <= 0",
            "removeCondition": "srd/cursed",
        }))
        .unwrap();
        let value = threshold_frame_write(&threshold);
        assert_eq!(
            value["ForEachActorInRadius"]["body"][0]["RemoveCondition"]["source_match"],
            json!("Any")
        );
    }

    #[test]
    fn threshold_remove_condition_with_source_uses_exact_match() {
        let threshold: ClockThresholdInput = serde_json::from_value(json!({
            "when": "actor.stat('srd/curse_stacks') <= 0",
            "removeCondition": "srd/cursed",
            "removeConditionSource": "srd/curse_source",
        }))
        .unwrap();
        let value = threshold_frame_write(&threshold);
        assert_eq!(
            value["ForEachActorInRadius"]["body"][0]["RemoveCondition"]["source_match"],
            json!({ "Exact": "srd/curse_source" })
        );
    }

    #[test]
    fn full_clock_frame_writes_orders_tallies_before_thresholds() {
        let frame_writes = clock_frame_writes(&death_saves_tallies(), &death_saves_thresholds());
        assert_eq!(frame_writes.len(), 5);
        assert!(frame_writes[0].get("ModifyResource").is_some());
        assert!(frame_writes[1].get("ModifyResource").is_some());
        assert!(frame_writes[2].get("ModifyResource").is_some());
        assert!(frame_writes[3].get("ForEachActorInRadius").is_some());
        assert!(frame_writes[4].get("ForEachActorInRadius").is_some());
    }

    #[test]
    fn empty_tallies_and_thresholds_yield_empty_frame_writes() {
        let frame_writes = clock_frame_writes(&[], &[]);
        assert!(frame_writes.is_empty());
    }
}
