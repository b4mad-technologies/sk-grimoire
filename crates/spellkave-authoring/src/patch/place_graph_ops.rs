//! Slice 3.5 (#1968 R7, ADR-0087 task 3.5): the place-graph group —
//! `add-place-graph`, `add-place-node`, `add-place-edge`. Ported verbatim
//! from `patch.ts`, including its u64ish-field handling for the H3 cell
//! fields (`origin_cell_res10/7/4` on `add-place-graph`, `cell_anchor` on
//! `add-place-edge` — `add-place-node` exposes NO cell fields in `patch.ts`
//! despite `PlaceNodeRow` carrying them, an existing asymmetry this port
//! preserves rather than "fixes").

use serde_json::Value;

use super::support::{
    optional_bool_field, optional_number_field, optional_string_field, optional_u64ish_field,
    patched_tables, remove_rows_by_field, resolved_bundle_id, row, row_array, s, string_field,
    string_field_or, PatchResult,
};

/// Mirrors `PlaceGraphRow`. Upsert by `place_graph_id`. `anchor_node_id` is
/// load-bearing (ADR-0029 Postscript 2026-05-27 despawn eject) and required
/// here, stricter than cross-row validation — that stays with the validator
/// (ADR-0023), this op does only presence/shape.
pub fn add_place_graph(manifest: &Value, payload: &Value) -> PatchResult<Value> {
    let bundle_id = resolved_bundle_id(manifest, payload);
    let place_graph_id = string_field(payload, "place_graph_id")?;
    let substrate_id = string_field(payload, "substrate_id")?;
    let display_name = string_field(payload, "display_name")?;
    let anchor_node_id = string_field(payload, "anchor_node_id")?;
    let description = optional_string_field(payload, "description");
    let bearing = optional_number_field(payload, "bearing");
    let metres_per_unit = optional_number_field(payload, "metres_per_unit");
    let parent_graph_id = optional_string_field(payload, "parent_graph_id");
    let blueprint_id = optional_string_field(payload, "blueprint_id");
    let origin_cell_res10 = optional_u64ish_field(payload, "origin_cell_res10");
    let origin_cell_res7 = optional_u64ish_field(payload, "origin_cell_res7");
    let origin_cell_res4 = optional_u64ish_field(payload, "origin_cell_res4");

    let mut rows = remove_rows_by_field(
        row_array(manifest, "place_graphs"),
        "place_graph_id",
        &place_graph_id,
    );
    let mut fields = vec![
        ("place_graph_id", s(place_graph_id)),
        ("bundle_id", s(bundle_id)),
        ("substrate_id", s(substrate_id)),
        ("display_name", s(display_name)),
        ("anchor_node_id", s(anchor_node_id)),
    ];
    if let Some(v) = description {
        fields.push(("description", s(v)));
    }
    if let Some(v) = bearing {
        fields.push(("bearing", v));
    }
    if let Some(v) = metres_per_unit {
        fields.push(("metres_per_unit", v));
    }
    if let Some(v) = parent_graph_id {
        fields.push(("parent_graph_id", s(v)));
    }
    if let Some(v) = blueprint_id {
        fields.push(("blueprint_id", s(v)));
    }
    if let Some(v) = origin_cell_res10 {
        fields.push(("origin_cell_res10", v));
    }
    if let Some(v) = origin_cell_res7 {
        fields.push(("origin_cell_res7", v));
    }
    if let Some(v) = origin_cell_res4 {
        fields.push(("origin_cell_res4", v));
    }
    rows.push(row(fields));
    Ok(patched_tables([("place_graphs", rows)]))
}

/// Mirrors `PlaceNodeRow`.
/// `node_id` is an explicit authored field (the composite-string PK,
/// `"<place_graph_id>/<local_id>"`) — this op does NOT synthesize it;
/// well-formedness is the validator's `node_id_well_formed` check, not this
/// op's job. Upsert by `node_id`.
pub fn add_place_node(manifest: &Value, payload: &Value) -> PatchResult<Value> {
    let bundle_id = resolved_bundle_id(manifest, payload);
    let node_id = string_field(payload, "node_id")?;
    let place_graph_id = string_field(payload, "place_graph_id")?;
    let local_id = string_field(payload, "local_id")?;
    let substrate_id = string_field(payload, "substrate_id")?;
    let display_name = string_field(payload, "display_name")?;
    let description = optional_string_field(payload, "description");
    let pos_x = optional_number_field(payload, "pos_x");
    let pos_y = optional_number_field(payload, "pos_y");
    let pos_present = optional_bool_field(payload, "pos_present");
    let pose_elevation = optional_number_field(payload, "pose_elevation");
    let pose_elevation_present = optional_bool_field(payload, "pose_elevation_present");
    // Authored extent shape reference (place-node-extent, D-1); empty =
    // resolve via the owning bundle's default extent. Replaces the removed
    // cell-denominated size_w/size_h.
    let extent_shape_id = optional_string_field(payload, "extent_shape_id");
    let extent_params_json = optional_string_field(payload, "extent_params_json");
    // Outdoor H3 membership (u64-ish strings OK — same path as graph origin cells).
    let cell_res10 = optional_u64ish_field(payload, "cell_res10");
    let cell_res7 = optional_u64ish_field(payload, "cell_res7");
    let cell_res4 = optional_u64ish_field(payload, "cell_res4");

    let mut rows = remove_rows_by_field(row_array(manifest, "place_nodes"), "node_id", &node_id);
    let mut fields = vec![
        ("node_id", s(node_id)),
        ("bundle_id", s(bundle_id)),
        ("place_graph_id", s(place_graph_id)),
        ("local_id", s(local_id)),
        ("substrate_id", s(substrate_id)),
        ("display_name", s(display_name)),
    ];
    if let Some(v) = description {
        fields.push(("description", s(v)));
    }
    if let Some(v) = pos_x {
        fields.push(("pos_x", v));
    }
    if let Some(v) = pos_y {
        fields.push(("pos_y", v));
    }
    if let Some(v) = pos_present {
        fields.push(("pos_present", Value::Bool(v)));
    }
    if let Some(v) = pose_elevation {
        fields.push(("pose_elevation", v));
    }
    if let Some(v) = pose_elevation_present {
        fields.push(("pose_elevation_present", Value::Bool(v)));
    }
    if let Some(v) = extent_shape_id {
        fields.push(("extent_shape_id", s(v)));
    }
    if let Some(v) = extent_params_json {
        fields.push(("extent_params_json", s(v)));
    }
    if let Some(v) = cell_res10 {
        fields.push(("cell_res10", v));
    }
    if let Some(v) = cell_res7 {
        fields.push(("cell_res7", v));
    }
    if let Some(v) = cell_res4 {
        fields.push(("cell_res4", v));
    }
    rows.push(row(fields));
    Ok(patched_tables([("place_nodes", rows)]))
}

/// Mirrors `PlaceEdgeRow`.
/// Upsert by `edge_key`, NEVER `edge_id` — `edge_id` is `#[auto_inc]`,
/// server-assigned, and not part of `PlaceEdgeRow` at all, so this op never
/// reads or writes it even if a caller's payload happens to carry one.
/// `place_graph_id` IS required as a field (the schema row has no
/// `#[serde(default)]` for it) but an EMPTY value is legal and means a
/// cross-graph edge — so it is guarded by presence-with-default, not a
/// non-empty requirement. Endpoint (from/to node) resolution is deliberately
/// NOT validated here — that is the validator's job.
///
/// ADR-0101 D-1: `inherit_movement_default` (optional bool, same
/// omit-when-absent convention as `substrate_crossing`) toggles whether this
/// edge receives the resolved `movement_default` mode set. Like every other
/// optional field on this op, re-authoring an edge WITHOUT repeating a
/// previously-set flag drops it back to the schema default (`true`) on the
/// re-rendered row — this is a WHOLE-ROW upsert-by-`edge_key`, not a
/// partial field patch (`add_place_edge_affordance`'s own doc comment notes
/// the same shape for `place_edge_affordance`). A caller that wants to flip
/// ONLY this flag without repeating the edge's full geometry should read the
/// existing row back out of its own workspace manifest and re-submit it with
/// every field it wants to keep, rather than expect a partial merge here.
pub fn add_place_edge(manifest: &Value, payload: &Value) -> PatchResult<Value> {
    let bundle_id = resolved_bundle_id(manifest, payload);
    let edge_key = string_field(payload, "edge_key")?;
    let place_graph_id = string_field_or(payload, "place_graph_id", "");
    let from_node_id = string_field(payload, "from_node_id")?;
    let to_node_id = string_field(payload, "to_node_id")?;
    let description = optional_string_field(payload, "description");
    let pose_dt_elevation = optional_number_field(payload, "pose_dt_elevation");
    let pose_dt_elevation_present = optional_bool_field(payload, "pose_dt_elevation_present");
    let substrate_crossing = optional_bool_field(payload, "substrate_crossing");
    let inherit_movement_default = optional_bool_field(payload, "inherit_movement_default");
    let cell_anchor = optional_u64ish_field(payload, "cell_anchor");
    let cell_anchor_u = optional_number_field(payload, "cell_anchor_u");
    let cell_anchor_v = optional_number_field(payload, "cell_anchor_v");
    let cell_anchor_present = optional_bool_field(payload, "cell_anchor_present");
    let from_anchor_side = optional_string_field(payload, "from_anchor_side");
    let from_anchor_offset = optional_number_field(payload, "from_anchor_offset");
    let to_anchor_side = optional_string_field(payload, "to_anchor_side");
    let to_anchor_offset = optional_number_field(payload, "to_anchor_offset");

    let mut rows = remove_rows_by_field(row_array(manifest, "place_edges"), "edge_key", &edge_key);
    let mut fields = vec![
        ("edge_key", s(edge_key)),
        ("bundle_id", s(bundle_id)),
        ("place_graph_id", s(place_graph_id)),
        ("from_node_id", s(from_node_id)),
        ("to_node_id", s(to_node_id)),
    ];
    if let Some(v) = description {
        fields.push(("description", s(v)));
    }
    if let Some(v) = pose_dt_elevation {
        fields.push(("pose_dt_elevation", v));
    }
    if let Some(v) = pose_dt_elevation_present {
        fields.push(("pose_dt_elevation_present", Value::Bool(v)));
    }
    if let Some(v) = substrate_crossing {
        fields.push(("substrate_crossing", Value::Bool(v)));
    }
    if let Some(v) = inherit_movement_default {
        fields.push(("inherit_movement_default", Value::Bool(v)));
    }
    if let Some(v) = cell_anchor {
        fields.push(("cell_anchor", v));
    }
    if let Some(v) = cell_anchor_u {
        fields.push(("cell_anchor_u", v));
    }
    if let Some(v) = cell_anchor_v {
        fields.push(("cell_anchor_v", v));
    }
    if let Some(v) = cell_anchor_present {
        fields.push(("cell_anchor_present", Value::Bool(v)));
    }
    if let Some(v) = from_anchor_side {
        fields.push(("from_anchor_side", s(v)));
    }
    if let Some(v) = from_anchor_offset {
        fields.push(("from_anchor_offset", v));
    }
    if let Some(v) = to_anchor_side {
        fields.push(("to_anchor_side", s(v)));
    }
    if let Some(v) = to_anchor_offset {
        fields.push(("to_anchor_offset", v));
    }
    rows.push(row(fields));
    Ok(patched_tables([("place_edges", rows)]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn empty_manifest() -> Value {
        json!({ "schema_version": "1.0.0" })
    }

    /// sk-gre: `add_place_edge` never read/wrote `description`, unlike its
    /// `add_place_graph`/`add_place_node` siblings in this same file, even
    /// though `PlaceEdgeRow` (`spellkave-schema`) carries the column and
    /// `spellkave_link_places` always passes the param.
    #[test]
    fn add_place_edge_threads_description_onto_the_row() {
        let manifest = empty_manifest();
        let payload = json!({
            "edge_key": "test/edge/a-b",
            "from_node_id": "test/a",
            "to_node_id": "test/b",
            "description": "a drafty doorway",
        });
        let out = add_place_edge(&manifest, &payload).expect("op should succeed");
        let rows = out["place_edges"].as_array().expect("array");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["description"], json!("a drafty doorway"));
    }

    /// Omitted description ⇒ key absent on the row (same convention as every
    /// other optional field on this op — see the `pose_dt_elevation`, etc.
    /// branches above).
    #[test]
    fn add_place_edge_omits_description_when_absent() {
        let manifest = empty_manifest();
        let payload = json!({
            "edge_key": "test/edge/a-b",
            "from_node_id": "test/a",
            "to_node_id": "test/b",
        });
        let out = add_place_edge(&manifest, &payload).expect("op should succeed");
        let rows = out["place_edges"].as_array().expect("array");
        assert!(rows[0].get("description").is_none());
    }
}
