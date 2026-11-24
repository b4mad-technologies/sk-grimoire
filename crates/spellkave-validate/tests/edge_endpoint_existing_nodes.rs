//! Black-box tests for `validate_manifest_with_existing_nodes` (#1666):
//! the edge-endpoint check (MF-VAL-105) resolves against already-committed
//! `place_node` ids in addition to the manifest's own `place_nodes`, while
//! the plain `validate_manifest` keeps the closed-world behavior for offline
//! callers (CLI merge-gate, wasm bridge).

use spellkave_schema::{BundleRow, Manifest, PlaceEdgeRow, SchemaVersion};
use spellkave_validate::{
    validate_manifest, validate_manifest_with_existing_nodes, MF_VAL_EDGE_ENDPOINT,
};
use std::collections::BTreeSet;

fn edge_patch_manifest() -> Manifest {
    let mut m = Manifest {
        schema_version: SchemaVersion::new(1, 0, 0),
        ..Manifest::default()
    };
    m.bundles.push(BundleRow {
        bundle_id: "patch".to_owned(),
        display_name: "patch".to_owned(),
        author: "test".to_owned(),
        version: "1.0.0".to_owned(),
        depends_on: vec![],
        description: String::new(),
        tag_prefixes: Vec::new(),
        allow_flat_tags: false,
        default_extent_shape_id: String::new(),
        default_extent_params_json: String::new(),
    });
    // The #1666 shape: an edge whose endpoints exist only in the live world
    // (e.g. one side created by an earlier place_graph_instances apply).
    m.place_edges.push(PlaceEdgeRow {
        edge_key: "trail_to_instance".to_owned(),
        bundle_id: "patch".to_owned(),
        place_graph_id: String::new(),
        from_node_id: "triboar_trail/ambush_site".to_owned(),
        to_node_id: "triboar_wooded_bend#s01/entry".to_owned(),
        pose_dt_elevation: 0.0,
        pose_dt_elevation_present: false,
        substrate_crossing: false,
        cell_anchor: 0,
        cell_anchor_u: 0.0,
        cell_anchor_v: 0.0,
        cell_anchor_present: false,
        from_anchor_side: String::new(),
        from_anchor_offset: 0.0,
        to_anchor_side: String::new(),
        to_anchor_offset: 0.0,
        description: String::new(),

        inherit_movement_default: true,
    });
    m
}

fn edge_endpoint_codes(diags: &[spellkave_validate::Diagnostic]) -> usize {
    diags
        .iter()
        .filter(|d| d.code == MF_VAL_EDGE_ENDPOINT)
        .count()
}

#[test]
fn closed_world_still_rejects_unknown_endpoints() {
    let diags = validate_manifest(&edge_patch_manifest());
    assert_eq!(edge_endpoint_codes(&diags), 1, "diags: {diags:?}");
}

#[test]
fn existing_nodes_resolve_edge_endpoints() {
    let existing: BTreeSet<String> = [
        "triboar_trail/ambush_site".to_owned(),
        "triboar_wooded_bend#s01/entry".to_owned(),
    ]
    .into();
    let diags = validate_manifest_with_existing_nodes(&edge_patch_manifest(), &existing);
    assert_eq!(edge_endpoint_codes(&diags), 0, "diags: {diags:?}");
}

#[test]
fn partially_resolved_edge_still_rejects() {
    // Only one endpoint is live — the edge must still fail.
    let existing: BTreeSet<String> = ["triboar_trail/ambush_site".to_owned()].into();
    let diags = validate_manifest_with_existing_nodes(&edge_patch_manifest(), &existing);
    assert_eq!(edge_endpoint_codes(&diags), 1, "diags: {diags:?}");
}
