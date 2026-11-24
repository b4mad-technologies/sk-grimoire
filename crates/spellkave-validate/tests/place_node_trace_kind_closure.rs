//! Checks authored place-node trace references against the available vocabulary.
//! The cases follow `edge_endpoint_existing_nodes.rs` for the sibling
//! `MF-VAL-105` edge-endpoint check.

use spellkave_schema::{
    BundleRow, Manifest, PlaceNodeTraceRow, SchemaVersion, TraceKindDefinitionRow,
};
use spellkave_validate::{validate_manifest, MF_VAL_PLACE_NODE_TRACE_KIND};

fn bundle_row() -> BundleRow {
    BundleRow {
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
    }
}

fn trace_kind_row(kind_id: &str) -> TraceKindDefinitionRow {
    TraceKindDefinitionRow {
        kind_id: kind_id.to_owned(),
        bundle_id: "patch".to_owned(),
        location_kinds: "place_node".to_owned(),
        schema_json: String::new(),
        perceptible_for_ticks: None,
        description: "a test trace kind".to_owned(),
    }
}

fn place_node_trace_row(node_id: &str, trace_kind_id: &str) -> PlaceNodeTraceRow {
    PlaceNodeTraceRow {
        node_id: node_id.to_owned(),
        trace_kind_id: trace_kind_id.to_owned(),
    }
}

fn place_node_trace_kind_codes(diags: &[spellkave_validate::Diagnostic]) -> usize {
    diags
        .iter()
        .filter(|d| d.code == MF_VAL_PLACE_NODE_TRACE_KIND)
        .count()
}

#[test]
fn unknown_trace_kind_id_is_rejected() {
    let manifest = Manifest {
        schema_version: SchemaVersion::new(1, 0, 0),
        bundles: vec![bundle_row()],
        place_node_traces: vec![place_node_trace_row("patch/hall", "patch/never_declared")],
        ..Manifest::default()
    };
    let diags = validate_manifest(&manifest);
    assert_eq!(place_node_trace_kind_codes(&diags), 1, "diags: {diags:?}");
    let hit = diags
        .iter()
        .find(|d| d.code == MF_VAL_PLACE_NODE_TRACE_KIND)
        .unwrap();
    assert!(hit.message.contains("patch/never_declared"));
}

#[test]
fn trace_kind_declared_in_the_same_manifest_resolves() {
    let manifest = Manifest {
        schema_version: SchemaVersion::new(1, 0, 0),
        bundles: vec![bundle_row()],
        trace_kind_definitions: vec![trace_kind_row("patch/scent")],
        place_node_traces: vec![place_node_trace_row("patch/hall", "patch/scent")],
        ..Manifest::default()
    };
    let diags = validate_manifest(&manifest);
    assert_eq!(place_node_trace_kind_codes(&diags), 0, "diags: {diags:?}");
}

#[test]
fn one_declared_kind_does_not_mask_a_sibling_undeclared_one() {
    let manifest = Manifest {
        schema_version: SchemaVersion::new(1, 0, 0),
        bundles: vec![bundle_row()],
        trace_kind_definitions: vec![trace_kind_row("patch/scent")],
        place_node_traces: vec![
            place_node_trace_row("patch/hall", "patch/scent"),
            place_node_trace_row("patch/bar", "patch/never_declared"),
        ],
        ..Manifest::default()
    };
    let diags = validate_manifest(&manifest);
    assert_eq!(place_node_trace_kind_codes(&diags), 1, "diags: {diags:?}");
}

#[test]
fn a_bootstrap_trace_kind_needs_no_manifest_declaration() {
    // Bootstrap kinds such as `scrape_marks` need no authored declaration.
    let manifest = Manifest {
        schema_version: SchemaVersion::new(1, 0, 0),
        bundles: vec![bundle_row()],
        place_node_traces: vec![place_node_trace_row("patch/hall", "scrape_marks")],
        ..Manifest::default()
    };
    let diags = validate_manifest(&manifest);
    assert_eq!(place_node_trace_kind_codes(&diags), 0, "diags: {diags:?}");
}
