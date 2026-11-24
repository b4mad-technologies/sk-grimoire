//! Authors see MF-VAL-327 for offers with identical niche, outcome and eligibility.
//! Reads the committed duplicate-offer fixture through Cargo. Spec: sk-j8e.3 / BL-39.

use spellkave_validate::{validate_manifest, MF_VAL_DUPLICATE_NICHE_OUTCOME_TEMPLATE};
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
fn duplicate_niche_outcome_fixture_emits_mf_val_327() {
    let source = fixture("duplicate_niche_outcome.toml");
    let parsed = spellkave_format::parse_manifest(&source).expect("fixture should parse");
    let diags = validate_manifest(&parsed.manifest);
    let hits: Vec<_> = diags
        .iter()
        .filter(|d| d.code == MF_VAL_DUPLICATE_NICHE_OUTCOME_TEMPLATE)
        .collect();
    assert_eq!(
        hits.len(),
        1,
        "expected one MF-VAL-327 on the duplicate-door fixture; got {diags:?}"
    );
    assert!(hits[0].message.contains("dup/door_a"));
    assert!(hits[0].message.contains("dup/door_b"));
    assert!(hits[0].message.contains("dup/door"));
    assert!(hits[0].message.contains("dup/open"));
}
