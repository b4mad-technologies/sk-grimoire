//! Authors can format living mechanics manifests without losing their rows.
//! Reads the committed mechanics fixtures through Cargo. Spec: sk-3o9.

use std::path::PathBuf;

use spellkave_format::{parse_manifest, render_manifest};

#[test]
fn render_round_trips_real_world_fixtures() {
    let repo_root = PathBuf::from(
        std::env::var_os("CARGO_MANIFEST_DIR")
            .expect("cargo test must supply the current package directory"),
    )
    .join("..")
    .join("..");
    let fixtures: [(&str, &str); 2] = [
        ("core", "packages/spellkave-mechanics/fixtures/core"),
        (
            "core-baseline",
            "packages/spellkave-mechanics/fixtures-baseline",
        ),
    ];
    for (name, rel) in fixtures {
        let path = repo_root.join(rel).join("manifest.toml");
        let source = std::fs::read_to_string(&path).unwrap_or_else(|err| {
            panic!(
                "sk-3o9: living fixture {name} missing at {}: {err}",
                path.display()
            )
        });
        let parsed = parse_manifest(&source)
            .unwrap_or_else(|err| panic!("{name} manifest should parse: {err:?}"));
        let rendered = render_manifest(&parsed.manifest)
            .unwrap_or_else(|err| panic!("{name} manifest should render: {err:?}"));
        let reparsed = parse_manifest(&rendered)
            .unwrap_or_else(|err| panic!("rendered {name} manifest should parse: {err:?}"));
        assert_eq!(
            reparsed.manifest, parsed.manifest,
            "{name} manifest must round-trip through render without loss"
        );
    }
}
