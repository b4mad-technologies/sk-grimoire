A grimoire is the book that explains how to write spells and other content for Spellkave.

# sk-grimoire

This Rust workspace defines Spellkave's content language and lets authors
parse, edit and validate manifests offline. The DSL crate handles rule
expressions. The schema, format and validation crates handle typed manifests,
TOML and diagnostics. The authoring crate provides composition, patches and
diffs. The two Wasm crates expose validation and authoring to JavaScript hosts.

The pinned Rust toolchain is selected by rustup. Run the checks from the
workspace root.

```sh
cargo build --locked --workspace --all-features --target-dir target
cargo test --locked --workspace --all-features --target-dir target
cargo clippy --locked --workspace --all-targets --all-features --target-dir target -- -D warnings
cargo fmt --all --check
cargo build --locked --release --target-dir target --target wasm32-unknown-unknown -p spellkave-validate-wasm -p spellkave-authoring-wasm
```

The Wasm build produces `spellkave_validate_wasm.wasm` and
`spellkave_authoring_wasm.wasm` in `target/wasm32-unknown-unknown/release/`.
The workspace and its frozen fixture data are documented in [PROVENANCE.md](PROVENANCE.md).

The software is licensed under [PolyForm Shield 1.0.0](LICENSE). See [NOTICE](NOTICE)
for attribution.
