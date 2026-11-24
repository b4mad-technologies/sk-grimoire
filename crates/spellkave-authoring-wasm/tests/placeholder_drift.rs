use wasm_bindgen_test::wasm_bindgen_test;

/// Mirrors `spellkave-validate-wasm/tests/multiruntime_drift.rs`'s shape: a
/// `wasm-pack test --node` suite proving the wasm-bound export matches what a
/// native caller would see. Slice 1 has no manifest logic to compare yet, so
/// this only proves the harness itself runs; later slices grow this file the
/// same way `multiruntime_drift.rs` grew alongside `spellkave-validate-wasm`.
#[wasm_bindgen_test]
fn placeholder_export_is_stable() {
    assert_eq!(
        spellkave_authoring_wasm::authoring_wasm_placeholder(),
        "spellkave-authoring-wasm:scaffold-ok"
    );
}

/// #1968 R7 slice 2 (ADR-0087): proves the wasm-bound row-kind table export
/// matches the native `spellkave-authoring::ROW_KIND_NATURAL_KEY` table
/// exactly — the wasm/native parity half of the migration's proof (the
/// wasm-vs-old-TS-literal half lives in `packages/spellkave-world-author`'s
/// own test suite, per the frozen-oracle invariant).
#[wasm_bindgen_test]
fn row_kind_natural_key_json_matches_native_table() {
    let json = spellkave_authoring_wasm::row_kind_natural_key_json();
    let value: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");
    let obj = value.as_object().expect("object");
    assert_eq!(obj.len(), spellkave_authoring::ROW_KIND_NATURAL_KEY.len());
    for (kind, field) in spellkave_authoring::ROW_KIND_NATURAL_KEY {
        assert_eq!(obj.get(*kind).and_then(|v| v.as_str()), Some(*field));
    }
}
