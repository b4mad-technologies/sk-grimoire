//! Checks the inline bootstrap and frozen mechanics fixtures.

use std::fs;
use std::path::{Path, PathBuf};

fn workspace_root() -> PathBuf {
    PathBuf::from(
        std::env::var_os("CARGO_MANIFEST_DIR")
            .expect("cargo test must supply the current package directory"),
    )
    .parent()
    .and_then(Path::parent)
    .expect("workspace root")
    .to_path_buf()
}

#[test]
fn core_bootstrap_stays_inline_parseable_and_filesystem_free_for_wasm() {
    let workspace = workspace_root();
    let source =
        fs::read_to_string(workspace.join("crates/spellkave-schema/src/core_bootstrap.rs"))
            .expect("read schema bootstrap module");
    let prefix = "pub const CORE_BOOTSTRAP_MANIFEST: &str = r#\"";
    let bootstrap = source
        .split_once(prefix)
        .and_then(|(_, rest)| rest.split_once("\"#;"))
        .map(|(manifest, _)| manifest)
        .expect("extract inline CORE_BOOTSTRAP_MANIFEST raw string");

    let parsed = spellkave_format::parse_manifest(bootstrap)
        .expect("inline CORE_BOOTSTRAP_MANIFEST must parse");
    assert!(
        parsed.manifest.bundles.is_empty(),
        "spatial bootstrap owns the core bundle row; manifest bootstrap adds vocabulary only"
    );
    assert!(parsed.manifest.capability_definitions.len() >= 3);
    assert!(parsed
        .manifest
        .capability_definitions
        .iter()
        .all(|row| row.bundle_id == "core"));
    assert!(bootstrap.contains("#:schema https://spellkave.dev/schema/world-manifest-1.0.0.json"));
    assert!(
        !source.contains("include_str!("),
        "wasm bootstrap must not acquire a repository-file dependency"
    );
    assert!(
        !source.contains("std::fs"),
        "wasm manifest runtime must stay filesystem-free"
    );
}

#[test]
fn reusable_mechanics_fixtures_parse() {
    let workspace = workspace_root();
    for path in [
        "packages/spellkave-mechanics/fixtures/mech_overheard_content_carriage/manifest.toml",
        "packages/spellkave-mechanics/fixtures/mech_spatial_inspect/manifest.toml",
        "packages/spellkave-mechanics/fixtures/role_capability_write_gate/manifest.toml",
    ] {
        let source = fs::read_to_string(workspace.join(path)).expect("read mechanics fixture");
        spellkave_format::parse_manifest(&source)
            .unwrap_or_else(|error| panic!("{path} should parse: {error}"));
    }
}

#[test]
fn core_presence_definition_matches_the_generated_fixture_without_seeds() {
    let bootstrap = spellkave_format::parse_manifest(spellkave_schema::CORE_BOOTSTRAP_MANIFEST)
        .unwrap()
        .manifest;
    let fixture: toml::Value = toml::from_str(
        &fs::read_to_string(
            workspace_root()
                .join("packages/spellkave-mechanics/fixtures/core/base/affordances.toml"),
        )
        .unwrap(),
    )
    .unwrap();
    let fixture_presence: spellkave_schema::EntityDefinitionRow = fixture["entity_definitions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["entity_def_id"].as_str() == Some("core/presence"))
        .unwrap()
        .clone()
        .try_into()
        .unwrap();
    let presence = bootstrap
        .entity_definitions
        .iter()
        .find(|r| r.entity_def_id == "core/presence")
        .unwrap();
    assert_eq!(&fixture_presence, presence);
    assert!(bootstrap
        .entity_definition_tag
        .iter()
        .all(|r| r.entity_def_id != "core/presence"));
    assert!(bootstrap
        .entity_definition_initial_capability
        .iter()
        .all(|r| r.entity_def_id != "core/presence"));
    assert!(bootstrap
        .entity_definition_initial_state
        .iter()
        .all(|r| r.entity_def_id != "core/presence"));
}
