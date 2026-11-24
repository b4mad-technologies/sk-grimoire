//! A create-only trace delta can refer to a node adopted without an upsert.
use spellkave_schema::{Manifest, PlaceNodeTraceRow};
use spellkave_validate::{validate_manifest_with_existing_nodes, Severity};
use std::collections::BTreeSet;

fn delta() -> Manifest {
    Manifest {
        place_node_traces: vec![PlaceNodeTraceRow {
            node_id: "adventure/entrance".into(),
            trace_kind_id: "scrape_marks".into(),
        }],
        ..Manifest::default()
    }
}

#[test]
fn trace_delta_accepts_an_existing_adopted_node() {
    let diagnostics = validate_manifest_with_existing_nodes(
        &delta(),
        &BTreeSet::from(["adventure/entrance".into()]),
    );
    assert!(
        diagnostics.iter().all(|d| d.severity != Severity::Fatal),
        "live node closure must validate without reapplying the node: {diagnostics:?}"
    );
}

#[test]
fn trace_delta_rejects_a_dangling_node_even_with_other_live_nodes() {
    let diagnostics =
        validate_manifest_with_existing_nodes(&delta(), &BTreeSet::from(["other/entrance".into()]));
    assert!(diagnostics
        .iter()
        .any(|d| { d.severity == Severity::Fatal && d.path == "place_node_traces.node_id" }));
}
