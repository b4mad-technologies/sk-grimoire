//! Slice 3.2 (#1968 R7, ADR-0087 task 3.2): the simple
//! upsert-by-single-field group — `add-entity`, `add-actor`,
//! `add-entity-definition`, `add-entity-tag`, `add-object-state-definition`.
//! Ported verbatim from `patch.ts`: same upsert keys, same cascades, same
//! `add-entity-tag` append-only exception (it is the one op in this group
//! that never upserts).

use serde_json::Value;

use super::support::{
    optional_bool_field, optional_number_field, optional_string_field, patched_tables,
    remove_rows_by_field, resolved_bundle_id, row, row_array, s, string_field, string_field_or,
    PatchResult,
};

/// `EntityRow` — #1602: no `#[auto_inc]`/server-assigned column, every field
/// is authorable. Upsert by `entity_key` so a bulk import (e.g. the PHB
/// pregen driver) can re-author idempotently.
pub fn add_entity(manifest: &Value, payload: &Value) -> PatchResult<Value> {
    let bundle_id = resolved_bundle_id(manifest, payload);
    let entity_key = string_field(payload, "entity_key")?;
    let name = string_field(payload, "name")?;
    let kind = string_field(payload, "kind")?;
    let place_node_id = string_field_or(payload, "place_node_id", "");
    let entity_def_id = optional_string_field(payload, "entity_def_id");
    let quantity = optional_number_field(payload, "quantity");
    let owned_by_entity_key = optional_string_field(payload, "owned_by_entity_key");
    // #1968: authored description — a first-class, never load-bearing column.
    let description = optional_string_field(payload, "description");

    let mut rows = remove_rows_by_field(row_array(manifest, "entities"), "entity_key", &entity_key);
    let mut fields = vec![
        ("entity_key", s(entity_key)),
        ("bundle_id", s(bundle_id)),
        ("name", s(name)),
        ("kind", s(kind)),
        ("place_node_id", s(place_node_id)),
    ];
    if let Some(v) = entity_def_id {
        fields.push(("entity_def_id", s(v)));
    }
    if let Some(v) = quantity {
        fields.push(("quantity", v));
    }
    if let Some(v) = owned_by_entity_key {
        fields.push(("owned_by_entity_key", s(v)));
    }
    if let Some(v) = description {
        fields.push(("description", s(v)));
    }
    rows.push(row(fields));
    Ok(patched_tables([("entities", rows)]))
}

/// `ActorRow` (key: `entity_key`) — #1602 condition 4: a re-authored actor
/// replaces rather than duplicates, cascading to its
/// `actor_spatial_locations` sidecar so a bulk re-import can move an actor
/// without leaving a stale location behind. The sidecar is ALWAYS touched
/// (the removal runs unconditionally, even when this call authors no
/// location), so both tables are always present in the output.
pub fn add_actor(manifest: &Value, payload: &Value) -> PatchResult<Value> {
    let bundle_id = resolved_bundle_id(manifest, payload);
    let entity_key = string_field(payload, "entity_key")?;
    let name = string_field(payload, "name")?;
    let description = optional_string_field(payload, "description");
    let place_node_id =
        optional_string_field(payload, "place_node_id").filter(|value| !value.is_empty());
    let selection_path = optional_string_field(payload, "selection_path");

    let mut actors = remove_rows_by_field(row_array(manifest, "actors"), "entity_key", &entity_key);
    let mut actor_fields = vec![
        ("entity_key", s(entity_key.clone())),
        ("bundle_id", s(bundle_id.clone())),
        ("name", s(name)),
    ];
    if let Some(v) = description {
        actor_fields.push(("description", s(v)));
    }
    actors.push(row(actor_fields));

    let mut locations = remove_rows_by_field(
        row_array(manifest, "actor_spatial_locations"),
        "entity_key",
        &entity_key,
    );
    if let Some(place_node_id) = place_node_id {
        let selection_path = selection_path.unwrap_or_else(|| format!("manifest:{entity_key}"));
        locations.push(row([
            ("entity_key", s(entity_key)),
            ("place_node_id", s(place_node_id)),
            ("bundle_id", s(bundle_id)),
            ("selection_path", s(selection_path)),
        ]));
    }
    Ok(patched_tables([
        ("actors", actors),
        ("actor_spatial_locations", locations),
    ]))
}

/// `EntityDefinitionRow` — upsert by `entity_def_id`, cascading to its tags
/// (mirrors `add-roll-table`'s cascade shape): a re-authored definition
/// replaces both its row and its tag rows rather than appending duplicates.
pub fn add_entity_definition(manifest: &Value, payload: &Value) -> PatchResult<Value> {
    let bundle_id = resolved_bundle_id(manifest, payload);
    let entity_def_id = string_field(payload, "entity_def_id")?;
    let display_name = string_field(payload, "display_name")?;
    let slot_compatibility_predicate =
        optional_string_field(payload, "slot_compatibility_predicate");
    let stackable = optional_bool_field(payload, "stackable");
    let tags = payload
        .get("tags")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let source = string_field_or(payload, "source", "manifest");

    let mut defs = remove_rows_by_field(
        row_array(manifest, "entity_definitions"),
        "entity_def_id",
        &entity_def_id,
    );
    let mut def_fields = vec![
        ("entity_def_id", s(entity_def_id.clone())),
        ("bundle_id", s(bundle_id.clone())),
        ("display_name", s(display_name)),
    ];
    if let Some(v) = slot_compatibility_predicate {
        def_fields.push(("slot_compatibility_predicate", s(v)));
    }
    if let Some(v) = stackable {
        def_fields.push(("stackable", Value::Bool(v)));
    }
    defs.push(row(def_fields));

    let mut tag_rows = remove_rows_by_field(
        row_array(manifest, "entity_definition_tag"),
        "entity_def_id",
        &entity_def_id,
    );
    for tag in &tags {
        let Some(tag_str) = tag.as_str() else {
            continue;
        };
        if tag_str.is_empty() {
            continue;
        }
        tag_rows.push(row([
            ("entity_def_id", s(entity_def_id.clone())),
            ("tag", s(tag_str.to_owned())),
            ("bundle_id", s(bundle_id.clone())),
            ("source", s(source.clone())),
        ]));
    }
    Ok(patched_tables([
        ("entity_definitions", defs),
        ("entity_definition_tag", tag_rows),
    ]))
}

/// `EntityTagRow` — the one op in this group that is append-only, NOT an
/// upsert (`patch.ts` never filters `entity_tag` before pushing).
pub fn add_entity_tag(manifest: &Value, payload: &Value) -> PatchResult<Value> {
    let bundle_id = resolved_bundle_id(manifest, payload);
    let entity_key = string_field(payload, "entity_key")?;
    let tag = string_field(payload, "tag")?;
    let source = string_field_or(payload, "source", "manifest");

    let mut rows = row_array(manifest, "entity_tag");
    rows.push(row([
        ("entity_key", s(entity_key)),
        ("tag", s(tag)),
        ("bundle_id", s(bundle_id)),
        ("source", s(source)),
    ]));
    Ok(patched_tables([("entity_tag", rows)]))
}

/// `ObjectStateDefinitionRow` — state vocabulary (e.g. `srd/hp`). Upsert by
/// `state_id` so the vocab can be re-declared idempotently by a bulk author.
pub fn add_object_state_definition(manifest: &Value, payload: &Value) -> PatchResult<Value> {
    let bundle_id = resolved_bundle_id(manifest, payload);
    let state_id = string_field(payload, "state_id")?;
    let display_name = string_field(payload, "display_name")?;
    let target_kind = string_field_or(payload, "target_kind", "entity");
    let default_visibility = string_field_or(payload, "default_visibility", "Public");

    let mut rows = remove_rows_by_field(
        row_array(manifest, "object_state_definitions"),
        "state_id",
        &state_id,
    );
    rows.push(row([
        ("state_id", s(state_id)),
        ("bundle_id", s(bundle_id)),
        ("target_kind", s(target_kind)),
        ("display_name", s(display_name)),
        ("default_visibility", s(default_visibility)),
    ]));
    Ok(patched_tables([("object_state_definitions", rows)]))
}
