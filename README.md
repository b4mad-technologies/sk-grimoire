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

## Diagnostic codes

[`diagnostic-codes.json`](diagnostic-codes.json) lists the codes emitted by the
parser, manifest validation, authoring, and NPC reference checks. The workspace tests keep it
in sync with the Rust registry. To regenerate it after an intentional change,
run the following command from the repository root.

```sh
SPELLKAVE_WRITE_DIAGNOSTIC_CODES=1 cargo test --locked --target-dir target \
  -p spellkave-validate --test no_duplicate_diagnostic_codes generated_diagnostic_code_list_matches_registry
```

## Wasm npm packages

Install both packages at an exact sk-grimoire release version. The first
planned release is `v0.1.0`.

```sh
npm install --save-exact @spellkave/validate-wasm@0.1.0 @spellkave/authoring-wasm@0.1.0
```

The package root selects the Node build in Node and the web build in browser
bundlers that resolve the `browser` export condition. Both packages expose
`/node` and `/browser` for explicit selection. The Node build initializes
automatically and supports `require` and named ESM imports.

```js
import { validate_manifest_toml } from '@spellkave/validate-wasm';
import { applyset_root_info_json } from '@spellkave/authoring-wasm';

const manifest = '#:schema https://spellkave.dev/schema/world-manifest-1.0.0.json\nschema_version = "1.0.0"\n';
const diagnostics = JSON.parse(validate_manifest_toml(manifest));
const applyset = JSON.parse(applyset_root_info_json(manifest));
```

The browser build exports an asynchronous initializer.

```js
import initValidator, { validate_manifest_toml } from '@spellkave/validate-wasm/browser';
import initAuthoring, { applyset_root_info_json } from '@spellkave/authoring-wasm/browser';

await Promise.all([initValidator(), initAuthoring()]);
```

By default, the browser glue fetches its Wasm file next to the JavaScript
module. To host the binary elsewhere, pass `{ module_or_path: wasmUrl }` to the
initializer. Both packages export the raw binaries as `/node/wasm` and
`/browser/wasm` for asset copying or explicit initialization.

### Build and test npm tarballs

Use Node 24.22.1, the Rust toolchain in `rust-toolchain.toml` and the
wasm-bindgen CLI matching `Cargo.lock`.

```sh
cargo install wasm-bindgen-cli --version '=0.2.120' --locked --target-dir target
node scripts/build-wasm-npm.mjs
node scripts/test-wasm-npm.mjs
```

The tarballs go under `target/wasm-npm/<version>/`. The test installs them in an empty
temporary project and runs both packages in Node and headless Chrome. Set
`CHROME_BIN` if Chrome is installed under a different command or path.
Both scripts accept a version argument and default to `0.0.0-ci`.

### Release from GitHub Actions

Both packages use the same version, taken from the sk-grimoire tag
`vMAJOR.MINOR.PATCH` with the leading `v` removed. PRs and branch pushes
build and test the packages. On release tags, the workflow publishes the
tested tarballs to npm with provenance.

For initial setup, each package must exist on npm before configuring its
trusted publisher. A maintainer can use
[staged publishing](https://docs.npmjs.com/staged-publishing/) to create each
package with npm's placeholder version, using tarballs built with
`node scripts/build-wasm-npm.mjs 0.0.0-bootstrap`. Keep that version staged
so the first release can come from GitHub Actions.

On npmjs.com, open **Packages → package → Settings → Trusted publishing**
for each package and select **GitHub Actions**. Enter `b4mad-technologies`
as the organization, `sk-grimoire` as the repository and `wasm-npm.yml` as
the workflow filename. Leave the environment field empty, enable direct
`npm publish` under allowed actions, and save. Complete the first release
within two days of adding the configurations. See the
[npm Trusted Publishing documentation](https://docs.npmjs.com/trusted-publishers/).

The software is licensed under [PolyForm Shield 1.0.0](LICENSE). See [NOTICE](NOTICE)
for attribution.
