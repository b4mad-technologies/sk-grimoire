// Authors receive stable diagnostics for invalid manifest rows and references.
// Uses in-memory manifests and committed diagnostic fixtures through Cargo.
// keep-file(K1): diagnostic classification, canonicalization and schema rules.
use spellkave_schema::{Manifest, MetadataValue, SchemaVersion, TargetConditionRow};
use spellkave_validate::{
    canonicalize_diagnostics, diagnostics_to_canonical_json, validate_manifest,
    validate_manifest_version, Diagnostic, MF_VAL_BUNDLE_DAG, MF_VAL_EDGE_ENDPOINT,
    MF_VAL_FK_CLOSURE, MF_VAL_H3_ANCESTRY, MF_VAL_LOCATION_PARENT_DAG,
    MF_VAL_METADATA_LEGACY_VALUE, MF_VAL_NODE_ID_FORMAT, MF_VAL_POSE_CHECK,
    MF_VAL_SUBSTRATE_CROSSING_ANCHOR, MF_VAL_UNSUPPORTED_SCHEMA_VERSION,
};
use std::path::PathBuf;

fn fixture(name: &str) -> String {
    let mut path = PathBuf::from(
        std::env::var_os("CARGO_MANIFEST_DIR")
            .expect("cargo test must supply the current package directory"),
    );
    path.push("tests");
    path.push("fixtures");
    path.push(name);
    std::fs::read_to_string(path).expect("fixture should load")
}

#[test]
fn canonicalization_fixture_sorts_by_contract() {
    let raw = fixture("diagnostics_unsorted.json");
    let mut diagnostics: Vec<Diagnostic> =
        serde_json::from_str(&raw).expect("fixture should parse");
    canonicalize_diagnostics(&mut diagnostics);
    assert_eq!(diagnostics[0].code, "MF-VAL-001");
}

#[test]
fn mf_code_for_unsupported_version_is_stable() {
    let diagnostics = validate_manifest_version("2.0.0");
    assert_eq!(diagnostics[0].code, MF_VAL_UNSUPPORTED_SCHEMA_VERSION);
}

#[test]
fn invariant_diagnostics_are_emitted_and_canonicalized() {
    let manifest = Manifest {
        schema_version: SchemaVersion::new(1, 0, 0),
        metadata: [
            (
                "entities".to_owned(),
                MetadataValue::String("node:a".to_owned()),
            ),
            (
                "fk_refs".to_owned(),
                MetadataValue::String("edge:e1->node:missing".to_owned()),
            ),
            (
                "cross_substrate_links".to_owned(),
                MetadataValue::String("a->b".to_owned()),
            ),
            (
                "crossing_anchors".to_owned(),
                MetadataValue::String("x->y".to_owned()),
            ),
            (
                "poses".to_owned(),
                MetadataValue::String("node:a:1:2".to_owned()),
            ),
            (
                "nodes".to_owned(),
                MetadataValue::String("node:a,bad".to_owned()),
            ),
            (
                "edges".to_owned(),
                MetadataValue::String("edge1:node:a->node:missing".to_owned()),
            ),
            (
                "bundle_deps".to_owned(),
                MetadataValue::String("core->addon,addon->core".to_owned()),
            ),
            (
                "h3_ancestry".to_owned(),
                MetadataValue::String("abcd->wxyz".to_owned()),
            ),
            (
                "location_parents".to_owned(),
                MetadataValue::String("town->inn,inn->town".to_owned()),
            ),
        ]
        .into_iter()
        .collect(),
        bundles: Vec::new(),
        place_graphs: Vec::new(),
        place_nodes: Vec::new(),
        place_edges: Vec::new(),
        location_definitions: Vec::new(),
        place_node_traces: Vec::new(),
        location_traces: Vec::new(),
        ..Manifest::default()
    };

    let diagnostics = validate_manifest(&manifest);
    let codes: Vec<&str> = diagnostics.iter().map(|diag| diag.code.as_str()).collect();
    assert!(codes.contains(&MF_VAL_FK_CLOSURE));
    assert!(codes.contains(&MF_VAL_SUBSTRATE_CROSSING_ANCHOR));
    assert!(codes.contains(&MF_VAL_POSE_CHECK));
    assert!(codes.contains(&MF_VAL_NODE_ID_FORMAT));
    assert!(codes.contains(&MF_VAL_EDGE_ENDPOINT));
    assert!(codes.contains(&MF_VAL_BUNDLE_DAG));
    assert!(codes.contains(&MF_VAL_H3_ANCESTRY));
    assert!(codes.contains(&MF_VAL_LOCATION_PARENT_DAG));

    let json = diagnostics_to_canonical_json(&diagnostics)
        .expect("canonical serialization should succeed");
    let reparsed: Vec<Diagnostic> =
        serde_json::from_str(&json).expect("canonical output should be valid json");
    assert_eq!(reparsed.len(), diagnostics.len());
}

#[test]
fn structured_reserved_metadata_cannot_bypass_legacy_validation_or_profile_contract() {
    let manifest = Manifest {
        schema_version: SchemaVersion::new(1, 0, 0),
        metadata: [
            (
                "entity_ids".to_owned(),
                MetadataValue::Array(vec![MetadataValue::String("2".to_owned())]),
            ),
            (
                "profile".to_owned(),
                MetadataValue::Object(Default::default()),
            ),
        ]
        .into_iter()
        .collect(),
        target_conditions: vec![TargetConditionRow {
            target_kind: "entity".to_owned(),
            target_id: 2,
            condition_definition_id: "test/condition".to_owned(),
            source: String::new(),
            stack_count: 1,
            expires_at_tick: None,
        }],
        ..Manifest::default()
    };

    let diagnostics = validate_manifest(&manifest);
    assert!(diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == MF_VAL_METADATA_LEGACY_VALUE)
        .any(|diagnostic| diagnostic.path == "metadata.entity_ids"));
    assert!(diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == MF_VAL_METADATA_LEGACY_VALUE)
        .any(|diagnostic| diagnostic.path == "metadata.profile"));
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.code == MF_VAL_FK_CLOSURE && diagnostic.path == "target_conditions.target_id"
    }));
}
