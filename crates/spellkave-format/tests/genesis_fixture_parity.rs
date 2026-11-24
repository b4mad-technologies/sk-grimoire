//! Checks that the inline core bootstrap agrees with the frozen mechanics fixtures.
//!
//! Rows authored twice (kernel genesis and the human-legible copy under
//! `packages/spellkave-mechanics/fixtures/core/base/`) are a same-value upsert
//! no-op only while both copies stay field-identical.

use std::path::PathBuf;

use spellkave_format::{parse_manifest, parse_manifest_fragment};
use spellkave_schema::{Manifest, CORE_BOOTSTRAP_MANIFEST};

fn core_base(file: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../packages/spellkave-mechanics/fixtures/core/base")
        .join(file);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("read core fixture {}: {err}", path.display()))
}

fn genesis() -> Manifest {
    parse_manifest(CORE_BOOTSTRAP_MANIFEST)
        .expect("genesis bootstrap manifest parses")
        .manifest
}

/// A bundle fragment has no schema header of its own; wrap it the way the
/// bundle loader does.
fn wrapped_fragment(file: &str) -> Manifest {
    let wrapped = format!(
        "#:schema https://spellkave.dev/schema/world-manifest-1.0.0.json\nschema_version = \"1.0.0\"\n\n{}",
        core_base(file)
    );
    parse_manifest(&wrapped)
        .unwrap_or_else(|err| panic!("core fixture {file} parses: {err:?}"))
        .manifest
}

#[test]
fn genesis_capability_rows_match_the_core_fixture_copies() {
    let genesis = genesis();
    let fixture = wrapped_fragment("affordances.toml");
    for id in [
        "frontier.apply_manifest",
        "frontier.pace_time",
        "core/world_admin",
        "frontier.spawn_entity_definition",
    ] {
        let find = |m: &Manifest| {
            m.capability_definitions
                .iter()
                .find(|row| row.capability_id == id)
                .cloned()
        };
        let genesis_row = find(&genesis).unwrap_or_else(|| panic!("genesis missing '{id}'"));
        let fixture_row = find(&fixture).unwrap_or_else(|| panic!("core fixture missing '{id}'"));
        assert_eq!(
            genesis_row, fixture_row,
            "genesis and core-fixture copies of '{id}' must stay field-identical"
        );
    }
}

#[test]
fn genesis_condition_vocabulary_rows_match_the_core_fixture_copies() {
    let genesis = genesis();
    let fixture = wrapped_fragment("conditions.toml");
    let condition = |m: &Manifest| {
        m.condition_definitions
            .iter()
            .find(|row| row.condition_definition_id == "core/condition/controlled")
            .cloned()
            .expect("core/condition/controlled present")
    };
    assert_eq!(condition(&genesis), condition(&fixture));
    let stacking = |m: &Manifest| {
        m.stacking_discipline_definitions
            .iter()
            .find(|row| row.discipline_id == "core/stacking/single")
            .cloned()
            .expect("core/stacking/single present")
    };
    assert_eq!(stacking(&genesis), stacking(&fixture));
    let policy = |m: &Manifest| {
        m.condition_remove_policy_definitions
            .iter()
            .find(|row| row.policy_id == "core/remove_policy/independent_lifetime")
            .cloned()
            .expect("core/remove_policy/independent_lifetime present")
    };
    assert_eq!(policy(&genesis), policy(&fixture));
}

#[test]
fn genesis_role_rows_match_core_fixture_copies() {
    let genesis = genesis();
    let roles = parse_manifest_fragment(&core_base("roles.toml")).expect("core roles parse");
    assert_eq!(genesis.role_definitions, roles.role_definitions);
    assert_eq!(genesis.role_definitions.len(), 7);
    let frontier = |m: Manifest| {
        let mut rows: Vec<_> = m
            .capability_definitions
            .into_iter()
            .filter(|r| r.capability_id.starts_with("frontier."))
            .collect();
        rows.sort_by(|a, b| a.capability_id.cmp(&b.capability_id));
        rows
    };
    let generated = frontier(
        parse_manifest_fragment(&core_base("affordances.toml")).expect("core affordances parse"),
    );
    let seeded = frontier(genesis);
    assert_eq!(
        generated.len(),
        22,
        "each reserved definition has exactly one generated row"
    );
    assert_eq!(seeded, generated);
}
