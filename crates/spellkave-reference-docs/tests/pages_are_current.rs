//! Fails when a generated reference page differs from its sources.

#[test]
fn reference_pages_match_their_sources() {
    let drifted = spellkave_reference_docs::drifted(&spellkave_reference_docs::repository_root());
    assert!(
        drifted.is_empty(),
        "{drifted:?} out of date; run: cargo run --locked -p spellkave-reference-docs -- --write"
    );
}
