//! Slice 3.4 (#1968 R7, ADR-0087 task 3.4): the roll-table group —
//! `add-roll-table`, `add-roll-table-tag`, `add-roll-table-entry-tag`
//! (`add-roll-table-entry` itself was slice 3.1's no-bundle_id group).
//! Ported verbatim from `patch.ts`.

use serde_json::Value;

use super::support::{
    patched_tables, remove_rows_by_field, remove_rows_by_fields, resolved_bundle_id, row,
    row_array, s, string_field, string_field_or, PatchResult,
};

/// #1577 — canon-warden PASS-WITH-CONDITIONS. Upsert by `roll_table_id`,
/// cascading to its tags (mirrors `add-entity-definition`'s cascade shape):
/// a re-authored table replaces both its row and its tag rows rather than
/// appending duplicates, so a bulk import (e.g. the Foundry weapon-damage
/// importer) can run idempotently.
pub fn add_roll_table(manifest: &Value, payload: &Value) -> PatchResult<Value> {
    let bundle_id = resolved_bundle_id(manifest, payload);
    let roll_table_id = string_field(payload, "roll_table_id")?;
    let display_name = string_field(payload, "display_name")?;
    let result_kind = string_field(payload, "result_kind")?;
    let tags = payload
        .get("tags")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let source = string_field_or(payload, "source", "manifest");

    let mut tables = remove_rows_by_field(
        row_array(manifest, "roll_tables"),
        "roll_table_id",
        &roll_table_id,
    );
    tables.push(row([
        ("roll_table_id", s(roll_table_id.clone())),
        ("bundle_id", s(bundle_id.clone())),
        ("display_name", s(display_name)),
        ("result_kind", s(result_kind)),
    ]));

    let mut tag_rows = remove_rows_by_field(
        row_array(manifest, "roll_table_tag"),
        "roll_table_id",
        &roll_table_id,
    );
    for tag in &tags {
        let Some(tag_str) = tag.as_str() else {
            continue;
        };
        if tag_str.is_empty() {
            continue;
        }
        tag_rows.push(row([
            ("roll_table_id", s(roll_table_id.clone())),
            ("tag", s(tag_str.to_owned())),
            ("bundle_id", s(bundle_id.clone())),
            ("source", s(source.clone())),
        ]));
    }
    Ok(patched_tables([
        ("roll_tables", tables),
        ("roll_table_tag", tag_rows),
    ]))
}

/// #1577 — idempotent upsert by the (roll_table_id, tag, source) triple,
/// mirroring `add-capability-grant`, NOT the append-only `add-entity-tag`.
pub fn add_roll_table_tag(manifest: &Value, payload: &Value) -> PatchResult<Value> {
    let bundle_id = resolved_bundle_id(manifest, payload);
    let roll_table_id = string_field(payload, "roll_table_id")?;
    let tag = string_field(payload, "tag")?;
    let source = string_field_or(payload, "source", "manifest");

    let mut rows = remove_rows_by_fields(
        row_array(manifest, "roll_table_tag"),
        &[
            ("roll_table_id", roll_table_id.as_str()),
            ("tag", tag.as_str()),
            ("source", source.as_str()),
        ],
    );
    rows.push(row([
        ("roll_table_id", s(roll_table_id)),
        ("tag", s(tag)),
        ("bundle_id", s(bundle_id)),
        ("source", s(source)),
    ]));
    Ok(patched_tables([("roll_table_tag", rows)]))
}

/// #1577 operator scope decision — entry-tags ARE in scope (core/d20
/// precedent). Same idempotent-upsert shape as `add-roll-table-tag`, keyed
/// on (roll_table_entry_id, tag, source). Unlike `add-roll-table-entry`
/// itself (no `bundle_id`), this row DOES carry `bundle_id` via the shared
/// fallback.
pub fn add_roll_table_entry_tag(manifest: &Value, payload: &Value) -> PatchResult<Value> {
    let bundle_id = resolved_bundle_id(manifest, payload);
    let roll_table_entry_id = string_field(payload, "roll_table_entry_id")?;
    let tag = string_field(payload, "tag")?;
    let source = string_field_or(payload, "source", "manifest");

    let mut rows = remove_rows_by_fields(
        row_array(manifest, "roll_table_entry_tag"),
        &[
            ("roll_table_entry_id", roll_table_entry_id.as_str()),
            ("tag", tag.as_str()),
            ("source", source.as_str()),
        ],
    );
    rows.push(row([
        ("roll_table_entry_id", s(roll_table_entry_id)),
        ("tag", s(tag)),
        ("bundle_id", s(bundle_id)),
        ("source", s(source)),
    ]));
    Ok(patched_tables([("roll_table_entry_tag", rows)]))
}
