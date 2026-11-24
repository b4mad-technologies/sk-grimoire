//! Slice 3.1 (#1968 R7, ADR-0087 task 3.1): the no-`bundle_id`-field
//! group. Every row kind here has NO `bundle_id` column in
//! `spellkave-schema` — `deny_unknown_fields` rejects one — a trap
//! `patch.ts`'s own comments cite by exact schema line number for each row
//! (`RollTableEntryRow`, `PlaceEdgeAffordanceRow`, `PlaceNodeTraceRow`,
//! `EntityContainmentRow`, `ActorEquipmentSlotRow`). Ported verbatim: same
//! upsert-by-(natural or composite)-key semantics, same field guards.
//!
//! Unlike `patch.ts` (which mutates the live manifest object field-by-field,
//! interleaving validation with removal), every op here validates ALL
//! required fields FIRST and only then computes the upsert — so a rejected
//! call never partially mutates anything (a strict tightening over `patch.ts`,
//! which could remove-then-throw on a still-missing field deeper in a row
//! literal; no existing test observes that partial-mutation quirk, and this
//! crate's dispatcher only ever applies an op's output on success).

use serde_json::Value;

use super::support::{
    number_field, optional_string_field, patched_tables, remove_rows_by_field,
    remove_rows_by_fields, row, row_array, s, string_field, stringifiable_field, PatchResult,
};

/// `RollTableEntryRow` (`crates/spellkave-schema/src/lib.rs`) has no
/// `bundle_id`. Upsert by `roll_table_entry_id`; the id is never derived
/// here (an authored convention, not a schema rule).
pub fn add_roll_table_entry(manifest: &Value, payload: &Value) -> PatchResult<Value> {
    let roll_table_entry_id = string_field(payload, "roll_table_entry_id")?;
    let roll_table_id = string_field(payload, "roll_table_id")?;
    let authored_order = number_field(payload, "authored_order")?;
    let weight = number_field(payload, "weight")?;
    // Stringify regardless of result_kind — fixtures store result_ref = "1"
    // under result_kind = "int" (core/base/roll-tables.toml).
    let result_ref = stringifiable_field(payload, "result_ref")?;

    let mut rows = remove_rows_by_field(
        row_array(manifest, "roll_table_entries"),
        "roll_table_entry_id",
        &roll_table_entry_id,
    );
    rows.push(row([
        ("roll_table_entry_id", s(roll_table_entry_id)),
        ("roll_table_id", s(roll_table_id)),
        ("authored_order", authored_order),
        ("weight", weight),
        ("result_ref", s(result_ref)),
    ]));
    Ok(patched_tables([("roll_table_entries", rows)]))
}

/// `PlaceEdgeAffordanceRow` (`crates/spellkave-schema/src/lib.rs`) has no
/// `bundle_id` — same trap as `EntityContainmentRow`/`RollTableEntryRow`.
/// Upsert by the composite (edge_key, affordance_template_id, mode_id) —
/// ADR-0101 D-1/D-5 widened the key when `mode_id` was added: an edge may
/// legally carry several affordance templates (each pairing owns its own
/// `bindings_json`) AND the SAME template under two distinct modes, so a
/// two-field filter would wrongly collapse an OR-alternative mode into the
/// implicit-mode row. Edge/template existence is deliberately NOT checked
/// here — cross-row resolution is the validator's job.
///
/// `mode_id` itself is intentionally OMITTED from the emitted row (not
/// written as `mode_id: ""`) when absent from the payload — mirrors
/// `bindings_json`'s own omit-when-absent convention and keeps every
/// pre-ADR-0101 caller's rendered row byte-identical (no field growth). The
/// upsert match therefore treats a payload/row `mode_id` of "missing" and
/// "present but empty" as the SAME implicit default mode — `mode_id_key`
/// below folds both to `""` on both sides of the comparison, which the
/// generic `remove_rows_by_fields` helper (exact-field-presence match)
/// cannot express, hence the hand-rolled filter instead of that helper.
pub fn add_place_edge_affordance(manifest: &Value, payload: &Value) -> PatchResult<Value> {
    let edge_key = string_field(payload, "edge_key")?;
    let affordance_template_id = string_field(payload, "affordance_template_id")?;
    let bindings_json = optional_string_field(payload, "bindings_json");
    let mode_id = optional_string_field(payload, "mode_id");
    let mode_id_key = mode_id.as_deref().unwrap_or("");

    let mut rows: Vec<Value> = row_array(manifest, "place_edge_affordance")
        .into_iter()
        .filter(|existing| {
            let row_edge_key = existing
                .get("edge_key")
                .and_then(Value::as_str)
                .unwrap_or("");
            let row_template = existing
                .get("affordance_template_id")
                .and_then(Value::as_str)
                .unwrap_or("");
            let row_mode_id = existing
                .get("mode_id")
                .and_then(Value::as_str)
                .unwrap_or("");
            !(row_edge_key == edge_key
                && row_template == affordance_template_id
                && row_mode_id == mode_id_key)
        })
        .collect();
    let mut fields = vec![
        ("edge_key", s(edge_key)),
        ("affordance_template_id", s(affordance_template_id)),
    ];
    if let Some(bindings_json) = bindings_json {
        fields.push(("bindings_json", s(bindings_json)));
    }
    if let Some(mode_id) = mode_id {
        if !mode_id.is_empty() {
            fields.push(("mode_id", s(mode_id)));
        }
    }
    rows.push(row(fields));
    Ok(patched_tables([("place_edge_affordance", rows)]))
}

/// `PlaceNodeTraceRow` (`crates/spellkave-schema/src/lib.rs`) has no
/// `bundle_id` (same trap as above). Table array is `place_node_traces`
/// (plural, unlike `place_edge_affordance`) — the op name still mirrors the
/// table row singular. Upsert by the composite (node_id, trace_kind_id): one
/// node legally carries several trace kinds.
pub fn add_place_node_trace(manifest: &Value, payload: &Value) -> PatchResult<Value> {
    let node_id = string_field(payload, "node_id")?;
    let trace_kind_id = string_field(payload, "trace_kind_id")?;

    let mut rows = remove_rows_by_fields(
        row_array(manifest, "place_node_traces"),
        &[
            ("node_id", node_id.as_str()),
            ("trace_kind_id", trace_kind_id.as_str()),
        ],
    );
    rows.push(row([
        ("node_id", s(node_id)),
        ("trace_kind_id", s(trace_kind_id)),
    ]));
    Ok(patched_tables([("place_node_traces", rows)]))
}

/// `EntityContainmentRow` (`crates/spellkave-schema/src/lib.rs`) has no
/// `bundle_id` (same trap as `RollTableEntryRow`). Upsert by `entity_key` —
/// a re-authored containment MOVES the entity (a bulk import re-running
/// should relocate, not duplicate, a carried item); `container_entity_key`/
/// `slot_id`/`direction` are not part of the key. Presence/type only — no
/// cycle or container-exists check here (that is the validator's job).
pub fn add_entity_containment(manifest: &Value, payload: &Value) -> PatchResult<Value> {
    let entity_key = string_field(payload, "entity_key")?;
    let container_entity_key = string_field(payload, "container_entity_key")?;
    let slot_id = optional_string_field(payload, "slot_id");
    let direction = optional_string_field(payload, "direction");

    let mut rows = remove_rows_by_field(
        row_array(manifest, "entity_containment"),
        "entity_key",
        &entity_key,
    );
    let mut fields = vec![
        ("entity_key", s(entity_key)),
        ("container_entity_key", s(container_entity_key)),
    ];
    if let Some(slot_id) = slot_id {
        fields.push(("slot_id", s(slot_id)));
    }
    if let Some(direction) = direction {
        fields.push(("direction", s(direction)));
    }
    rows.push(row(fields));
    Ok(patched_tables([("entity_containment", rows)]))
}

/// `ActorEquipmentSlotRow` (`crates/spellkave-schema/src/lib.rs`) has no
/// `bundle_id` (same trap as above). Upsert by the composite
/// (actor_entity_key, slot_id) — the row's own natural key (an actor has at
/// most one item per slot). `slot_id` is a pass-through open-vocabulary
/// string (ADR-0018); this op does not validate slot-definition existence or
/// item/slot compatibility.
pub fn add_actor_equipment_slot(manifest: &Value, payload: &Value) -> PatchResult<Value> {
    let actor_entity_key = string_field(payload, "actor_entity_key")?;
    let slot_id = string_field(payload, "slot_id")?;
    let item_key = string_field(payload, "item_key")?;

    let mut rows = remove_rows_by_fields(
        row_array(manifest, "actor_equipment_slots"),
        &[
            ("actor_entity_key", actor_entity_key.as_str()),
            ("slot_id", slot_id.as_str()),
        ],
    );
    rows.push(row([
        ("actor_entity_key", s(actor_entity_key)),
        ("slot_id", s(slot_id)),
        ("item_key", s(item_key)),
    ]));
    Ok(patched_tables([("actor_equipment_slots", rows)]))
}
