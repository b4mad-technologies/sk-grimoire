//! Slice 3.3 (#1968 R7, ADR-0087 task 3.3): the capability/identity-grant
//! group (composite-key upserts) — `add-capability-grant`,
//! `add-actor-identity`, `add-entity-definition-initial-capability`,
//! `add-entity-definition-initial-state`. Ported verbatim from `patch.ts`:
//! same composite upsert keys (or single-field, for `add-actor-identity`'s
//! deliberate author-time simplification — see that function's own doc).

use serde_json::Value;

use super::support::{
    optional_bool_field, optional_number_field, optional_string_field, patched_tables,
    remove_rows_by_field, remove_rows_by_fields, resolved_bundle_id, row, row_array, s,
    string_field, string_field_or, PatchResult,
};

/// Brief 11 D10 / architecture §5.1 — `[[actor_capabilities]]` grants an
/// authored capability to an existing actor/entity instance. Upsert by
/// (actor_entity_key, capability_id, source): re-authoring the same grant
/// replaces rather than duplicates, mirroring
/// `add-entity-definition-initial-state`'s upsert-by-identity pattern.
pub fn add_capability_grant(manifest: &Value, payload: &Value) -> PatchResult<Value> {
    let actor_entity_key = string_field(payload, "actor_entity_key")?;
    let capability_id = string_field(payload, "capability_id")?;
    let source = string_field_or(payload, "source", "manifest");
    // ADR-0120 read-visibility override; ADR-0007 D-4 see-as scope. All
    // optional — an omitted `visibility` field falls back at manifest commit
    // to the granted capability's `capability_definition.default_visibility`
    // (itself `Public` when undeclared); an omitted `extent_ref` grants no
    // elevated visibility (#1935).
    let visibility = optional_string_field(payload, "visibility");
    let extent_ref = optional_string_field(payload, "extent_ref");
    let extent_kind_ref = optional_string_field(payload, "extent_kind_ref");
    let source_args_hash = optional_string_field(payload, "source_args_hash");

    let mut rows = remove_rows_by_fields(
        row_array(manifest, "actor_capabilities"),
        &[
            ("actor_entity_key", actor_entity_key.as_str()),
            ("capability_id", capability_id.as_str()),
            ("source", source.as_str()),
        ],
    );
    let mut fields = vec![
        ("actor_entity_key", s(actor_entity_key)),
        ("capability_id", s(capability_id)),
        ("source", s(source)),
        ("bundle_id", s(resolved_bundle_id(manifest, payload))),
    ];
    if let Some(v) = visibility {
        fields.push(("visibility", s(v)));
    }
    if let Some(v) = extent_ref {
        fields.push(("extent_ref", s(v)));
    }
    if let Some(v) = extent_kind_ref {
        fields.push(("extent_kind_ref", s(v)));
    }
    if let Some(v) = source_args_hash {
        fields.push(("source_args_hash", s(v)));
    }
    rows.push(row(fields));
    Ok(patched_tables([("actor_capabilities", rows)]))
}

/// #1968 verb-UX round 6 — `[[actor_identities]]` binds an existing actor to
/// a real or macro identity. Upsert by `entity_key` ALONE: the server itself
/// upserts on the compound `(actor_id, identity)`, but every authored
/// fixture/story in this repo binds one `entity_key` to exactly ONE identity
/// (verified corpus-wide, zero counter-examples), so `entity_key` is the
/// natural AUTHOR-TIME key — a re-authored binding for the same actor
/// replaces its identity/role rather than duplicating.
pub fn add_actor_identity(manifest: &Value, payload: &Value) -> PatchResult<Value> {
    let entity_key = string_field(payload, "entity_key")?;
    let identity = string_field(payload, "identity")?;
    let role = string_field_or(payload, "role", "primary");

    let mut rows = remove_rows_by_field(
        row_array(manifest, "actor_identities"),
        "entity_key",
        &entity_key,
    );
    rows.push(row([
        ("entity_key", s(entity_key)),
        ("identity", s(identity)),
        ("role", s(role)),
        ("bundle_id", s(resolved_bundle_id(manifest, payload))),
    ]));
    Ok(patched_tables([("actor_identities", rows)]))
}

/// #728 — bind a capability to a catalog DEFINITION so every instance
/// inherits it at birth (`apply_definition_seeds` reads this seed table),
/// rather than granting one actor instance directly. Upsert by
/// (entity_def_id, capability_id), mirroring
/// `add-entity-definition-initial-state`'s (entity_def_id, state_id) upsert.
pub fn add_entity_definition_initial_capability(
    manifest: &Value,
    payload: &Value,
) -> PatchResult<Value> {
    let entity_def_id = string_field(payload, "entity_def_id")?;
    let capability_id = string_field(payload, "capability_id")?;

    let mut rows = remove_rows_by_fields(
        row_array(manifest, "entity_definition_initial_capability"),
        &[
            ("entity_def_id", entity_def_id.as_str()),
            ("capability_id", capability_id.as_str()),
        ],
    );
    rows.push(row([
        ("entity_def_id", s(entity_def_id)),
        ("capability_id", s(capability_id)),
        ("bundle_id", s(resolved_bundle_id(manifest, payload))),
    ]));
    Ok(patched_tables([(
        "entity_definition_initial_capability",
        rows,
    )]))
}

/// #729 — a definition's initial state-value (one typed carrier per row).
/// Upsert by (entity_def_id, state_id) so a re-author replaces the value.
pub fn add_entity_definition_initial_state(
    manifest: &Value,
    payload: &Value,
) -> PatchResult<Value> {
    let entity_def_id = string_field(payload, "entity_def_id")?;
    let state_id = string_field(payload, "state_id")?;
    let value_int = optional_number_field(payload, "value_int");
    let value_bool = optional_bool_field(payload, "value_bool");
    let value_text = optional_string_field(payload, "value_text");

    let mut rows = remove_rows_by_fields(
        row_array(manifest, "entity_definition_initial_state"),
        &[
            ("entity_def_id", entity_def_id.as_str()),
            ("state_id", state_id.as_str()),
        ],
    );
    let mut fields = vec![
        ("entity_def_id", s(entity_def_id)),
        ("state_id", s(state_id)),
        ("bundle_id", s(resolved_bundle_id(manifest, payload))),
    ];
    if let Some(v) = value_int {
        fields.push(("value_int", v));
    }
    if let Some(v) = value_bool {
        fields.push(("value_bool", Value::Bool(v)));
    }
    if let Some(v) = value_text {
        fields.push(("value_text", s(v)));
    }
    rows.push(row(fields));
    Ok(patched_tables([("entity_definition_initial_state", rows)]))
}
