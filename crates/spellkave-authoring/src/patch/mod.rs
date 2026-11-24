//! `patch_manifest_json` — the Rust-owned counterpart of
//! `@spellkave/world-author`'s `patch.ts` (#1968 R7, ADR-0087, task
//! section 3). Ported incrementally, one op-group per slice
//! (`task` 3.1–3.6); each op group lives in its own module so a slice's
//! diff stays coherent. `mod.rs` is only the dispatcher + the op-group
//! module wiring — no manifest semantics live here directly.
//!
//! See `support`'s module doc for the shared read/write contract every op
//! follows (whole-manifest JSON in, only-the-changed-table(s) JSON out).

pub mod capability_identity_ops;
pub mod communicate_ops;
pub mod movement_ops;
pub mod no_bundle_id_ops;
pub mod place_graph_ops;
pub mod roll_table_ops;
pub mod simple_upsert_ops;
pub mod support;

use serde_json::Value;
use support::PatchResult;

/// Applies one authoring operation to a manifest, given as JSON. Returns a
/// JSON object of ONLY the table(s) the op mutated: `{"<table>": [...],
/// ...}` — see `support`'s module doc for why. Errors mirror `patch.ts`'s
/// thrown `Error` messages verbatim (same substrings, so existing
/// `toThrow(/regex/)` assertions keep matching); `spellkave-authoring-wasm`
/// crosses them to JS via `JsValue::from_str`, matching
/// `spellkave-validate-wasm`'s existing error convention.
pub fn patch_manifest_json(
    manifest_json: &str,
    op: &str,
    payload_json: &str,
) -> PatchResult<String> {
    let manifest: Value = serde_json::from_str(manifest_json)
        .map_err(|err| format!("patch manifest JSON parse failed: {err}"))?;
    let payload: Value = serde_json::from_str(payload_json)
        .map_err(|err| format!("patch payload JSON parse failed: {err}"))?;

    let patched = dispatch(&manifest, op, &payload)?;

    serde_json::to_string(&patched)
        .map_err(|err| format!("patch output JSON serialization failed: {err}"))
}

fn dispatch(manifest: &Value, op: &str, payload: &Value) -> PatchResult<Value> {
    match op {
        "add-roll-table-entry" => no_bundle_id_ops::add_roll_table_entry(manifest, payload),
        "add-place-edge-affordance" => {
            no_bundle_id_ops::add_place_edge_affordance(manifest, payload)
        }
        "add-place-node-trace" => no_bundle_id_ops::add_place_node_trace(manifest, payload),
        "add-entity-containment" => no_bundle_id_ops::add_entity_containment(manifest, payload),
        "add-actor-equipment-slot" => no_bundle_id_ops::add_actor_equipment_slot(manifest, payload),
        "add-entity" => simple_upsert_ops::add_entity(manifest, payload),
        "add-actor" => simple_upsert_ops::add_actor(manifest, payload),
        "add-entity-definition" => simple_upsert_ops::add_entity_definition(manifest, payload),
        "add-entity-tag" => simple_upsert_ops::add_entity_tag(manifest, payload),
        "add-object-state-definition" => {
            simple_upsert_ops::add_object_state_definition(manifest, payload)
        }
        "add-capability-grant" => capability_identity_ops::add_capability_grant(manifest, payload),
        "add-actor-identity" => capability_identity_ops::add_actor_identity(manifest, payload),
        "add-entity-definition-initial-capability" => {
            capability_identity_ops::add_entity_definition_initial_capability(manifest, payload)
        }
        "add-entity-definition-initial-state" => {
            capability_identity_ops::add_entity_definition_initial_state(manifest, payload)
        }
        "add-roll-table" => roll_table_ops::add_roll_table(manifest, payload),
        "add-roll-table-tag" => roll_table_ops::add_roll_table_tag(manifest, payload),
        "add-roll-table-entry-tag" => roll_table_ops::add_roll_table_entry_tag(manifest, payload),
        "add-place-graph" => place_graph_ops::add_place_graph(manifest, payload),
        "add-place-node" => place_graph_ops::add_place_node(manifest, payload),
        "add-place-edge" => place_graph_ops::add_place_edge(manifest, payload),
        "add-communicate-affordance" => {
            communicate_ops::add_communicate_affordance(manifest, payload)
        }
        // ADR-0101 D-1/D-7.
        "add-movement-default" => movement_ops::add_movement_default(manifest, payload),
        _ => Err(format!("unknown patch op: {op}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn empty_manifest() -> String {
        json!({ "schema_version": "1.0.0" }).to_string()
    }

    #[test]
    fn add_roll_table_entry_upserts_by_entry_id() {
        let payload = json!({
            "roll_table_entry_id": "test/d6:1",
            "roll_table_id": "test/d6",
            "authored_order": 1,
            "weight": 1,
            "result_ref": 1,
        })
        .to_string();
        let out = patch_manifest_json(&empty_manifest(), "add-roll-table-entry", &payload)
            .expect("op should succeed");
        let parsed: Value = serde_json::from_str(&out).expect("output should be valid JSON");
        let entries = parsed["roll_table_entries"].as_array().expect("array");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0]["result_ref"], json!("1"));
        assert!(entries[0].get("bundle_id").is_none());
    }

    #[test]
    fn add_entity_containment_requires_container_entity_key() {
        let payload = json!({ "entity_key": "test/coin" }).to_string();
        let err = patch_manifest_json(&empty_manifest(), "add-entity-containment", &payload)
            .expect_err("missing container_entity_key should error");
        assert!(err.contains("container_entity_key"));
    }

    #[test]
    fn add_place_edge_affordance_dispatches_through_and_keeps_mode_id_siblings_distinct() {
        let manifest = empty_manifest();
        let walk = json!({
            "edge_key": "test/edge/a-b",
            "affordance_template_id": "core/affordance/step",
        })
        .to_string();
        let out = patch_manifest_json(&manifest, "add-place-edge-affordance", &walk)
            .expect("op should succeed");
        let fly = json!({
            "edge_key": "test/edge/a-b",
            "affordance_template_id": "core/affordance/step",
            "mode_id": "fly",
        })
        .to_string();
        let out2 = patch_manifest_json(&out, "add-place-edge-affordance", &fly)
            .expect("op should succeed");
        let merged: Value = serde_json::from_str(&out2).expect("valid JSON");
        let rows = merged["place_edge_affordance"].as_array().expect("array");
        // ADR-0101 D-1: same edge + template under two distinct modes are siblings, not
        // an upsert-replace — the widened (edge_key, template, mode_id) key at work.
        assert_eq!(rows.len(), 2);
        assert!(rows.iter().any(|r| r.get("mode_id").is_none()));
        assert!(rows.iter().any(|r| r["mode_id"] == json!("fly")));
    }

    #[test]
    fn add_movement_default_dispatches_through() {
        let payload = json!({
            "bundle_id": "srd",
            "affordance_template_id": "srd/affordance/step",
        })
        .to_string();
        let out = patch_manifest_json(&empty_manifest(), "add-movement-default", &payload)
            .expect("op should succeed");
        let parsed: Value = serde_json::from_str(&out).expect("output should be valid JSON");
        let rows = parsed["movement_defaults"].as_array().expect("array");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["mode_id"], json!(""));
    }

    #[test]
    fn unknown_op_errors() {
        let err = patch_manifest_json(&empty_manifest(), "not-a-real-op", "{}")
            .expect_err("unknown op should error");
        assert!(err.contains("unknown patch op: not-a-real-op"));
    }
}
