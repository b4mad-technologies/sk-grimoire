//! Slice 4.10 (#1968 R7, ADR-0087 task 4.10): `clusterRows` itself —
//! the final assembly `compose.ts` used to hand-write: the per-behavior
//! loop, the shape dispatch, and the remaining non-shape-specific row
//! pushes (`capability_definitions`, the `outcome_templates` SUCCESS row,
//! `action_definitions`/`action_definition_tag`, the `consequence_templates`
//! SUCCESS row, `consequence_frame_writes` assembly, `object_state` seeds).
//! Every SHAPE/modifier's own logic already lives in this crate's sibling
//! modules (`ids`, `live_wiring`, `single_target`, `aoe`, `modifiers`,
//! `zone`, `scheduled_zone`, `summon`, `outcome_graph`, `clock`, landed
//! task 4.1-4.9) — this module's own job is PURELY orchestration,
//! calling those already-tested functions in the SAME order `compose.ts`'s
//! `clusterRows` did, never re-deriving their logic.

use serde_json::{Map, Value};

use super::aoe::{aoe_frame_write, AoeBehaviorInput};
use super::clock::{clock_frame_writes, ClockBehaviorInput};
use super::ids::{compose_ids, IdsBehaviorInput, IdsConfigInput};
use super::live_wiring::{compose_live_wiring_json, LiveWiringConfigInput};
use super::modifiers::{
    affordance_predicate, attacks_frame_writes, resource_cost_debit_frame_write,
    AffordancePredicateInput, MultiAttackInput, ResourceCostInput,
};
use super::outcome_graph::{outcome_graph_tables, OutcomeGraphBehaviorInput};
use super::scheduled_zone::{schedule_autonomy_job_effect_definition, ScheduledZoneBehaviorInput};
use super::single_target::{single_target_frame_write, SingleTargetBehaviorInput};
use super::summon::{summon_effect_definition, SummonBehaviorInput};
use super::zone::{
    zone_marker_definition, zone_marker_inscribe, zone_pulse_frame_write, ZoneBehaviorInput,
};
use crate::patch::support::{row, s, PatchResult};

/// `BehaviorConfig.objectStateSeeds[]`.
struct ObjectStateSeed {
    entity_key: String,
    state_id: String,
    value_int: Value,
    source_template_id: String,
}

fn parse_object_state_seed(value: &Value) -> Option<ObjectStateSeed> {
    Some(ObjectStateSeed {
        entity_key: value.get("entityKey")?.as_str()?.to_owned(),
        state_id: value.get("stateId")?.as_str()?.to_owned(),
        value_int: value.get("valueInt")?.clone(),
        source_template_id: value.get("sourceTemplateId")?.as_str()?.to_owned(),
    })
}

/// A growing multi-table row accumulator — mirrors `compose.ts`'s `rows`
/// object literal (one `Vec<Value>` per table, all predefined so a table
/// key is always present to append into, dropped from the final output only
/// if it stayed empty).
struct RowSink {
    tables: std::collections::BTreeMap<&'static str, Vec<Value>>,
}

impl RowSink {
    fn new() -> Self {
        let mut tables = std::collections::BTreeMap::new();
        for table in [
            "capability_definitions",
            "capability_definition_tag",
            "outcome_templates",
            "affordance_templates",
            "action_definitions",
            "action_definition_tag",
            "consequence_templates",
            "consequence_frame_writes",
            "effect_definitions",
            "actors",
            "entities",
            "actor_spatial_locations",
            "actor_capabilities",
            "entity_definition_initial_capability",
            "object_state",
            "spatial_niche_attachments",
            "spatial_state_definitions",
            "outcome_graphs",
            "outcome_graph_nodes",
            "outcome_graph_edges",
        ] {
            tables.insert(table, Vec::new());
        }
        Self { tables }
    }

    fn push(&mut self, table: &'static str, value: Value) {
        self.tables.entry(table).or_default().push(value);
    }

    /// Merges a `{"<table>": [...], ...}` object (from a sub-module's own
    /// `_json` export) onto this sink — the SAME concat-append shape
    /// `compose.ts`'s live-wiring/outcome-graph merge loops use. Any table
    /// key not already predefined is admitted too (defensive; every actual
    /// sub-module return uses one of `RowSink::new`'s own keys).
    fn merge(&mut self, tables_json: &Value) {
        let Value::Object(map) = tables_json else {
            return;
        };
        for (table, rows) in map {
            let Value::Array(rows) = rows else { continue };
            let table_static: &'static str = match table.as_str() {
                "capability_definitions" => "capability_definitions",
                "capability_definition_tag" => "capability_definition_tag",
                "outcome_templates" => "outcome_templates",
                "affordance_templates" => "affordance_templates",
                "action_definitions" => "action_definitions",
                "action_definition_tag" => "action_definition_tag",
                "consequence_templates" => "consequence_templates",
                "consequence_frame_writes" => "consequence_frame_writes",
                "effect_definitions" => "effect_definitions",
                "actors" => "actors",
                "entities" => "entities",
                "actor_spatial_locations" => "actor_spatial_locations",
                "actor_capabilities" => "actor_capabilities",
                "entity_definition_initial_capability" => "entity_definition_initial_capability",
                "object_state" => "object_state",
                "spatial_niche_attachments" => "spatial_niche_attachments",
                "spatial_state_definitions" => "spatial_state_definitions",
                "outcome_graphs" => "outcome_graphs",
                "outcome_graph_nodes" => "outcome_graph_nodes",
                "outcome_graph_edges" => "outcome_graph_edges",
                _ => continue,
            };
            self.tables
                .entry(table_static)
                .or_default()
                .extend(rows.clone());
        }
    }

    /// Drops empty tables so the renderer doesn't emit bare headers — the
    /// same `Object.fromEntries(Object.entries(rows).filter(...))` tail
    /// `compose.ts`'s `clusterRows` ends with.
    fn into_value(self) -> Value {
        let mut map = Map::new();
        for (table, rows) in self.tables {
            if !rows.is_empty() {
                map.insert(table.to_owned(), Value::Array(rows));
            }
        }
        Value::Object(map)
    }
}

fn opt_str(behavior: &Value, key: &str) -> Option<String> {
    behavior.get(key).and_then(Value::as_str).map(str::to_owned)
}

fn opt_bool(behavior: &Value, key: &str) -> Option<bool> {
    behavior.get(key).and_then(Value::as_bool)
}

/// The whole `clusterRows(behaviors, cfg)` port. `behaviors_json` is a JSON
/// ARRAY of `ActorBehavior` objects; `cfg_json` is one `BehaviorConfig`
/// object. Returns the merged, non-empty-tables-only rows object, exactly
/// matching `compose.ts`'s return shape.
pub fn cluster_rows(behaviors_json: &str, cfg_json: &str) -> PatchResult<String> {
    let behaviors: Vec<Value> = serde_json::from_str(behaviors_json)
        .map_err(|err| format!("clusterRows behaviors JSON parse failed: {err}"))?;
    let cfg: Value = serde_json::from_str(cfg_json)
        .map_err(|err| format!("clusterRows cfg JSON parse failed: {err}"))?;

    let bundle_id = cfg
        .get("bundleId")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    let cap_prefix = cfg
        .get("capPrefix")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    let action_prefix = cfg
        .get("actionPrefix")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    let id_namespace = opt_str(&cfg, "idNamespace");

    let mut sink = RowSink::new();

    for behavior in &behaviors {
        let behavior_json = serde_json::to_string(behavior)
            .map_err(|err| format!("clusterRows behavior re-serialization failed: {err}"))?;

        let key = behavior
            .get("key")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
        let capability_id = opt_str(behavior, "capabilityId");
        let display_name = behavior
            .get("displayName")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
        let shape = behavior
            .get("shape")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();

        let ids_behavior = IdsBehaviorInput {
            key: key.clone(),
            capability_id: capability_id.clone(),
        };
        let ids_cfg = IdsConfigInput {
            cap_prefix: cap_prefix.clone(),
            action_prefix: action_prefix.clone(),
            id_namespace: id_namespace.clone(),
        };
        let ids = compose_ids(&ids_behavior, &ids_cfg);
        let cap = ids.cap;
        let niche = ids.niche;
        let action = ids.action;
        let outcome = ids.outcome;
        let affordance = ids.affordance;
        let success = ids.success;

        if opt_bool(behavior, "emitCapability").unwrap_or(false) {
            sink.push(
                "capability_definitions",
                row([
                    ("capability_id", s(cap.clone())),
                    ("bundle_id", s(bundle_id.clone())),
                    ("actor_kind", s("entity")),
                    ("display_name", s(display_name.clone())),
                    (
                        "description",
                        s(opt_str(behavior, "capabilityDescription").unwrap_or_default()),
                    ),
                ]),
            );
            if let Some(tags) = behavior.get("capabilityTags").and_then(Value::as_array) {
                for tag in tags.iter().filter_map(Value::as_str) {
                    sink.push(
                        "capability_definition_tag",
                        row([
                            ("capability_id", s(cap.clone())),
                            ("tag", s(tag.to_owned())),
                            ("bundle_id", s(bundle_id.clone())),
                            ("source", s("manifest")),
                        ]),
                    );
                }
            }
        }

        // outcome-graph: the action's outcome INVOKES the graph (the effect
        // is applied at the selected node, not by this consequence).
        let graph_id = format!("{action}/graph");
        let mut outcome_fields = vec![
            ("outcome_template_id", s(outcome.clone())),
            ("bundle_id", s(bundle_id.clone())),
            ("display_name", s(action.clone())),
            ("outcome_bool", Value::Bool(true)),
            ("manipulation_action_id", s(action.clone())),
        ];
        if let Some(speech_act) = opt_str(behavior, "communicationSpeechActId") {
            outcome_fields.push(("communication_speech_act_id", s(speech_act)));
        }
        // Carry the authored observation description for manifest validation.
        if let Some(observation) = opt_str(behavior, "observation") {
            outcome_fields.push(("description", s(observation)));
        }
        if shape == "outcome-graph" {
            outcome_fields.push((
                "outcome_expression_source",
                s(format!("invoke_outcome_graph('{graph_id}')")),
            ));
        }
        sink.push("outcome_templates", row(outcome_fields));

        // Affordance predicate — the shape-aware gate decision (task 4.4).
        let affordance_predicate_input: AffordancePredicateInput =
            serde_json::from_value(behavior.clone())
                .map_err(|err| format!("affordance predicate behavior parse failed: {err}"))?;
        let predicate_portion = affordance_predicate(&affordance_predicate_input);
        let mut affordance_fields = vec![
            ("template_id", s(affordance.clone())),
            ("bundle_id", s(bundle_id.clone())),
            ("niche_id", s(niche.clone())),
            ("capability_id", s(cap.clone())),
            ("display_name", s(display_name.clone())),
        ];
        // `affordance_predicate` returns EXACTLY one of these two known keys
        // (never a dynamic key) — match on them directly rather than
        // iterating the object generically, which would need a `&'static
        // str` for a runtime-derived key.
        if let Some(predicate_source) = predicate_portion.get("predicate_source") {
            affordance_fields.push(("predicate_source", predicate_source.clone()));
        } else if let Some(predicate_bool) = predicate_portion.get("predicate_bool") {
            affordance_fields.push(("predicate_bool", predicate_bool.clone()));
        }
        affordance_fields.push(("outcome_template_id", s(outcome.clone())));
        affordance_fields.push((
            "range_m",
            behavior
                .get("rangeM")
                .cloned()
                .unwrap_or_else(|| Value::from(0)),
        ));
        sink.push("affordance_templates", row(affordance_fields));

        // ONE canonical action_definition. `resolutionMode` /
        // `resolutionPredicateSource` / `onFailure` (#2075/#1891 composer-
        // resolution-fields scope contract) map 1:1 onto the
        // action_definitions columns of the same name (snake_case), mirroring
        // the shared `spellkave_define_action`/`spellkave_define_behavior`
        // params.
        // Row-level and shape-agnostic on purpose: they
        // sit beside the existing `selfEligible` field, which is emitted the
        // same way for every shape, and every shape shares this ONE
        // action_definitions push (there is no per-shape branch here). All
        // three stay OMITTED unless the behavior sets them, so an
        // already-authored behavior renders byte-identical output (the
        // schema's own column defaults — Deterministic / empty string —
        // apply downstream exactly as if the key were present and equal to
        // the default; the omission itself is the byte-stability contract,
        // not a functional difference).
        let mut action_fields = vec![
            ("action_id", s(action.clone())),
            ("bundle_id", s(bundle_id.clone())),
            ("required_capability", s(cap.clone())),
            ("on_success", s(success.clone())),
            ("display_name", s(display_name.clone())),
            ("cost_time_ms", Value::from(1000)),
            (
                "self_eligible",
                Value::Bool(opt_bool(behavior, "selfEligible").unwrap_or(false)),
            ),
        ];
        if let Some(resolution_mode) = opt_str(behavior, "resolutionMode") {
            action_fields.push(("resolution_mode", s(resolution_mode)));
        }
        if let Some(resolution_predicate_source) = opt_str(behavior, "resolutionPredicateSource") {
            action_fields.push((
                "resolution_predicate_source",
                s(resolution_predicate_source),
            ));
        }
        if let Some(on_failure) = opt_str(behavior, "onFailure") {
            action_fields.push(("on_failure", s(on_failure)));
        }
        sink.push("action_definitions", row(action_fields));
        if let Some(tags) = behavior.get("actionTags").and_then(Value::as_array) {
            for tag in tags.iter().filter_map(Value::as_str) {
                sink.push(
                    "action_definition_tag",
                    row([
                        ("action_id", s(action.clone())),
                        ("tag", s(tag.to_owned())),
                        ("bundle_id", s(bundle_id.clone())),
                        ("source", s("manifest")),
                    ]),
                );
            }
        }

        // scheduled-zone/summon reference their effect off effect_refs_csv.
        let schedule_effect_id = format!("{action}/schedule");
        let pulse_id = format!("{action}/pulse");
        let spawn_effect_id = format!("{action}/spawn");
        let mut success_fields = vec![
            ("template_id", s(success.clone())),
            ("bundle_id", s(bundle_id.clone())),
        ];
        if shape == "scheduled-zone" {
            success_fields.push(("effect_refs_csv", s(schedule_effect_id.clone())));
        }
        if shape == "summon" {
            success_fields.push(("effect_refs_csv", s(spawn_effect_id.clone())));
        }
        sink.push("consequence_templates", row(success_fields));

        let is_env_state = shape == "env-state";

        // Frame_write topology by shape — every branch reuses an
        // already-ported (task 4.1-4.9) sub-module function verbatim.
        let mut frame_writes: Vec<Value> = if let Some(attacks_value) = behavior.get("attacks") {
            let attacks: Vec<MultiAttackInput> = serde_json::from_value(attacks_value.clone())
                .map_err(|err| format!("attacks parse failed: {err}"))?;
            attacks_frame_writes(&attacks)
        } else if is_env_state || shape == "zone" || shape == "scheduled-zone" {
            let zone_behavior: ZoneBehaviorInput = serde_json::from_value(behavior.clone())
                .map_err(|err| format!("zone behavior parse failed: {err}"))?;
            sink.push(
                "spatial_state_definitions",
                zone_marker_definition(&zone_behavior, &bundle_id),
            );
            let inscribe = zone_marker_inscribe(&zone_behavior);
            if shape == "zone" {
                vec![inscribe, zone_pulse_frame_write(&zone_behavior)]
            } else if shape == "scheduled-zone" {
                let scheduled_zone_behavior: ScheduledZoneBehaviorInput =
                    serde_json::from_value(behavior.clone())
                        .map_err(|err| format!("scheduled-zone behavior parse failed: {err}"))?;
                sink.push(
                    "effect_definitions",
                    schedule_autonomy_job_effect_definition(
                        &scheduled_zone_behavior,
                        &action,
                        &bundle_id,
                    ),
                );
                sink.push(
                    "consequence_templates",
                    row([
                        ("template_id", s(pulse_id.clone())),
                        ("bundle_id", s(bundle_id.clone())),
                    ]),
                );
                sink.push(
                    "consequence_frame_writes",
                    row([
                        ("template_id", s(pulse_id.clone())),
                        ("ordinal", Value::from(0)),
                        ("frame_write", zone_pulse_frame_write(&zone_behavior)),
                    ]),
                );
                vec![inscribe]
            } else {
                vec![inscribe]
            }
        } else if shape == "summon" {
            let summon_behavior: SummonBehaviorInput = serde_json::from_value(behavior.clone())
                .map_err(|err| format!("summon behavior parse failed: {err}"))?;
            sink.push(
                "effect_definitions",
                summon_effect_definition(&summon_behavior, &spawn_effect_id, &bundle_id),
            );
            vec![]
        } else if shape == "outcome-graph" {
            let outcome_graph_behavior: OutcomeGraphBehaviorInput =
                serde_json::from_value(behavior.clone())
                    .map_err(|err| format!("outcome-graph behavior parse failed: {err}"))?;
            let tables = outcome_graph_tables(&outcome_graph_behavior, &graph_id, &bundle_id);
            sink.merge(&tables);
            vec![]
        } else if shape == "single-target" {
            let single_target_behavior: SingleTargetBehaviorInput =
                serde_json::from_value(behavior.clone())
                    .map_err(|err| format!("single-target behavior parse failed: {err}"))?;
            vec![single_target_frame_write(&single_target_behavior)]
        } else if shape == "clock" {
            let clock_behavior: ClockBehaviorInput = serde_json::from_value(behavior.clone())
                .map_err(|err| format!("clock behavior parse failed: {err}"))?;
            clock_frame_writes(
                &clock_behavior.clock_tallies.unwrap_or_default(),
                &clock_behavior.clock_thresholds.unwrap_or_default(),
            )
        } else {
            // "aoe" (default) shape.
            let aoe_behavior: AoeBehaviorInput = serde_json::from_value(behavior.clone())
                .map_err(|err| format!("aoe behavior parse failed: {err}"))?;
            vec![aoe_frame_write(&aoe_behavior)?]
        };

        // Resource-pool debit — the LAST ordinal, on the caster.
        if let Some(resource_cost_value) = behavior.get("resourceCost") {
            let cost: ResourceCostInput = serde_json::from_value(resource_cost_value.clone())
                .map_err(|err| format!("resourceCost parse failed: {err}"))?;
            frame_writes.push(resource_cost_debit_frame_write(&cost));
        }
        for (ordinal, frame_write) in frame_writes.into_iter().enumerate() {
            sink.push(
                "consequence_frame_writes",
                row([
                    ("template_id", s(success.clone())),
                    ("ordinal", Value::from(ordinal as i64)),
                    ("frame_write", frame_write),
                ]),
            );
        }

        // ── Live wiring ──
        let live_wiring_cfg = LiveWiringConfigInput {
            bundle_id: bundle_id.clone(),
            cap_prefix: cap_prefix.clone(),
            action_prefix: action_prefix.clone(),
            id_namespace: id_namespace.clone(),
            default_caster: cfg
                .get("defaultCaster")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned(),
            default_caster_node: cfg
                .get("defaultCasterNode")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned(),
        };
        let live_wiring_cfg_json = serde_json::to_string(&serde_json::json!({
            "bundleId": live_wiring_cfg.bundle_id,
            "capPrefix": live_wiring_cfg.cap_prefix,
            "actionPrefix": live_wiring_cfg.action_prefix,
            "idNamespace": live_wiring_cfg.id_namespace,
            "defaultCaster": live_wiring_cfg.default_caster,
            "defaultCasterNode": live_wiring_cfg.default_caster_node,
        }))
        .map_err(|err| format!("live-wiring cfg re-serialization failed: {err}"))?;
        let live_wiring_json = compose_live_wiring_json(&behavior_json, &live_wiring_cfg_json)?;
        let live_wiring: Value = serde_json::from_str(&live_wiring_json)
            .map_err(|err| format!("live-wiring output parse failed: {err}"))?;
        sink.merge(&live_wiring);
    }

    if let Some(seeds) = cfg.get("objectStateSeeds").and_then(Value::as_array) {
        for seed_value in seeds {
            if let Some(seed) = parse_object_state_seed(seed_value) {
                sink.push(
                    "object_state",
                    row([
                        ("entity_key", s(seed.entity_key)),
                        ("state_id", s(seed.state_id)),
                        ("value_int", seed.value_int),
                        ("source_template_id", s(seed.source_template_id)),
                        ("bundle_id", s(bundle_id.clone())),
                    ]),
                );
            }
        }
    }

    let value = sink.into_value();
    serde_json::to_string(&value)
        .map_err(|err| format!("clusterRows output JSON serialization failed: {err}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const BASE_CFG: &str = r#"{
        "bundleId": "srd",
        "manifestPath": "example/manifest.toml",
        "capPrefix": "test",
        "actionPrefix": "",
        "defaultCaster": "srd/regdar",
        "defaultCasterNode": "srd_outdoor/cell_a"
    }"#;

    #[test]
    fn single_target_behavior_expands_the_full_chain() {
        let behaviors = json!([{
            "key": "test-strike",
            "displayName": "Test Strike",
            "emitCapability": true,
            "actionTags": ["test"],
            "shape": "single-target",
            "predicateSource": "d20 >= 13",
            "deltaSource": "0 - 1d6",
        }])
        .to_string();
        let out = cluster_rows(&behaviors, BASE_CFG).expect("should succeed");
        let rows: Value = serde_json::from_str(&out).expect("valid JSON");
        for table in [
            "capability_definitions",
            "outcome_templates",
            "affordance_templates",
            "action_definitions",
            "consequence_templates",
            "consequence_frame_writes",
            "actor_capabilities",
            "spatial_niche_attachments",
        ] {
            assert_eq!(
                rows[table].as_array().map(Vec::len),
                Some(1),
                "table {table} should have exactly 1 row, got {:?}",
                rows[table]
            );
        }
        assert_eq!(
            rows["capability_definitions"][0]["capability_id"],
            json!("srd/test/test-strike")
        );
        assert_eq!(
            rows["action_definitions"][0]["required_capability"],
            json!("srd/test/test-strike")
        );
        assert_eq!(
            rows["affordance_templates"][0]["predicate_source"],
            json!("d20 >= 13")
        );
        assert_eq!(
            rows["consequence_frame_writes"][0]["frame_write"]["ModifyResource"]["delta_source"],
            json!("0 - 1d6")
        );
        // No resolution_mode/resolution_predicate_source/on_failure key is
        // present at all when the behavior doesn't set them (see the
        // byte-stability regression test below for the exact same assertion
        // restated as an explicit contract).
        let action_row = &rows["action_definitions"][0];
        assert!(action_row.get("resolution_mode").is_none());
        assert!(action_row.get("resolution_predicate_source").is_none());
        assert!(action_row.get("on_failure").is_none());
    }

    // #2075/#1891 composer-resolution-fields scope contract: a single-target
    // behavior that sets `resolutionMode`/`resolutionPredicateSource`/
    // `onFailure` gets all three carried onto the emitted action_definitions
    // row verbatim (snake_case, 1:1), alongside the ALREADY-battle-tested
    // "offer predicate is deterministic eligibility only" idiom (#1891): the
    // attack roll now lives in `resolutionPredicateSource`, not
    // `predicateSource`.
    #[test]
    fn single_target_behavior_carries_resolution_fields_onto_action_definitions() {
        let behaviors = json!([{
            "key": "goblin-scimitar",
            "displayName": "Goblin Scimitar",
            "emitCapability": true,
            "actionTags": ["srd", "monster-action"],
            "shape": "single-target",
            "predicateSource": "",
            "deltaSource": "0 - (1d6 + 2)",
            "resolutionMode": "Stochastic",
            "resolutionPredicateSource": "d20 >= 15",
            "onFailure": "srd/goblin-scimitar/miss",
        }])
        .to_string();
        let out = cluster_rows(&behaviors, BASE_CFG).expect("should succeed");
        let rows: Value = serde_json::from_str(&out).expect("valid JSON");
        let action_row = &rows["action_definitions"][0];
        assert_eq!(action_row["resolution_mode"], json!("Stochastic"));
        assert_eq!(
            action_row["resolution_predicate_source"],
            json!("d20 >= 15")
        );
        assert_eq!(action_row["on_failure"], json!("srd/goblin-scimitar/miss"));
        // The existing on_success surface is untouched by the new fields.
        assert_eq!(
            action_row["on_success"],
            json!("srd/goblin-scimitar/success")
        );
    }

    // Byte-stability regression (ledger contract §4: "omitted fields keep
    // today's exact output"): a behavior that predates this change (no
    // resolution* / onFailure keys at all) must still render an
    // action_definitions row with EXACTLY the pre-existing key set — no
    // resolution_mode/resolution_predicate_source/on_failure key sneaks in
    // with a default value. This pins the pre-change key set literally so a
    // future accidental "always emit with a default" edit fails loudly.
    #[test]
    fn action_definitions_row_is_byte_stable_when_resolution_fields_are_unset() {
        let behaviors = json!([{
            "key": "test-strike",
            "displayName": "Test Strike",
            "emitCapability": true,
            "actionTags": ["test"],
            "shape": "single-target",
            "predicateSource": "d20 >= 13",
            "deltaSource": "0 - 1d6",
        }])
        .to_string();
        let out = cluster_rows(&behaviors, BASE_CFG).expect("should succeed");
        let rows: Value = serde_json::from_str(&out).expect("valid JSON");
        assert_eq!(
            rows["action_definitions"][0],
            json!({
                "action_id": "srd/test-strike",
                "bundle_id": "srd",
                "required_capability": "srd/test/test-strike",
                "on_success": "srd/test-strike/success",
                "display_name": "Test Strike",
                "cost_time_ms": 1000,
                "self_eligible": false,
            })
        );
    }

    #[test]
    fn empty_tables_are_dropped_from_the_output() {
        let behaviors = json!([{
            "key": "test-strike",
            "displayName": "Test Strike",
            "emitCapability": true,
            "actionTags": ["test"],
            "shape": "single-target",
            "predicateSource": "",
            "deltaSource": "1d4",
        }])
        .to_string();
        let out = cluster_rows(&behaviors, BASE_CFG).expect("should succeed");
        let rows: Value = serde_json::from_str(&out).expect("valid JSON");
        assert!(rows.get("outcome_graphs").is_none());
        assert!(rows.get("effect_definitions").is_none());
        assert!(rows.get("entities").is_none());
    }

    #[test]
    fn merge_row_tables_concatenates_across_behaviors() {
        let behaviors = json!([
            {
                "key": "test-strike",
                "displayName": "Test Strike",
                "emitCapability": true,
                "actionTags": ["test"],
                "shape": "single-target",
                "predicateSource": "d20 >= 13",
                "deltaSource": "0 - 1d6",
            },
            {
                "key": "test-strike-2",
                "displayName": "Test Strike 2",
                "emitCapability": true,
                "actionTags": ["test"],
                "shape": "single-target",
                "predicateSource": "d20 >= 13",
                "deltaSource": "0 - 1d6",
            }
        ])
        .to_string();
        let out = cluster_rows(&behaviors, BASE_CFG).expect("should succeed");
        let rows: Value = serde_json::from_str(&out).expect("valid JSON");
        assert_eq!(rows["capability_definitions"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn resource_cost_debit_appends_after_body_and_gates_the_affordance() {
        let behaviors = json!([{
            "key": "fireball",
            "displayName": "Fireball",
            "emitCapability": true,
            "actionTags": ["spell"],
            "shape": "aoe",
            "radiusM": 20,
            "predicateSource": "",
            "deltaSource": "0 - 8d6",
            "resourceCost": { "resource": "srd/spell_slot_3", "amount": 1 },
        }])
        .to_string();
        let out = cluster_rows(&behaviors, BASE_CFG).expect("should succeed");
        let rows: Value = serde_json::from_str(&out).expect("valid JSON");
        let frame_writes = rows["consequence_frame_writes"].as_array().unwrap();
        assert_eq!(frame_writes.len(), 2);
        assert_eq!(
            frame_writes[1]["frame_write"]["ModifyResource"]["actor"],
            json!("Actor")
        );
        assert_eq!(
            frame_writes[1]["frame_write"]["ModifyResource"]["resource"],
            json!("srd/spell_slot_3")
        );
        assert_eq!(
            rows["affordance_templates"][0]["predicate_source"],
            json!("actor.stat('srd/spell_slot_3') >= 1")
        );
    }

    #[test]
    fn outcome_graph_behavior_merges_all_six_tables() {
        let behaviors = json!([{
            "key": "acid-arrow",
            "displayName": "Acid Arrow",
            "emitCapability": true,
            "actionTags": ["spell"],
            "shape": "outcome-graph",
            "predicateSource": "",
            "deltaSource": "",
            "branches": [
                { "node": "hit", "guard": "d20 >= 15", "deltaSource": "4d4" },
                { "node": "miss", "guard": "1 == 1", "deltaSource": "2d4" },
            ],
        }])
        .to_string();
        let out = cluster_rows(&behaviors, BASE_CFG).expect("should succeed");
        let rows: Value = serde_json::from_str(&out).expect("valid JSON");
        assert_eq!(rows["outcome_graphs"].as_array().unwrap().len(), 1);
        assert_eq!(rows["outcome_graph_nodes"].as_array().unwrap().len(), 3);
        assert_eq!(rows["outcome_graph_edges"].as_array().unwrap().len(), 2);
        // "outcome_templates" has the SUCCESS row (from clusterRows itself,
        // shape="outcome-graph") plus root + 2 branch outcome templates from
        // outcome_graph_tables — 4 total.
        assert_eq!(rows["outcome_templates"].as_array().unwrap().len(), 4);
        assert_eq!(
            rows["outcome_templates"][0]["outcome_expression_source"],
            json!("invoke_outcome_graph('srd/acid-arrow/graph')")
        );
        // outcome-graph shape has NO direct consequence_frame_writes of its
        // own (empty frameWrites) — only the per-branch ones from
        // outcome_graph_tables.
        assert_eq!(
            rows["consequence_frame_writes"].as_array().unwrap().len(),
            2
        );
    }

    #[test]
    fn object_state_seeds_from_cfg_are_appended() {
        let cfg = json!({
            "bundleId": "srd",
            "manifestPath": "example/manifest.toml",
            "capPrefix": "test",
            "actionPrefix": "",
            "defaultCaster": "srd/regdar",
            "defaultCasterNode": "srd_outdoor/cell_a",
            "objectStateSeeds": [
                {
                    "entityKey": "srd/regdar",
                    "stateId": "srd/hp",
                    "valueInt": 10,
                    "sourceTemplateId": "srd/seed",
                },
            ],
        })
        .to_string();
        let out = cluster_rows("[]", &cfg).expect("should succeed");
        let rows: Value = serde_json::from_str(&out).expect("valid JSON");
        assert_eq!(rows["object_state"][0]["entity_key"], json!("srd/regdar"));
        assert_eq!(rows["object_state"][0]["value_int"], json!(10));
    }
}
