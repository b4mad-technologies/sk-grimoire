# spellkave-validate-wasm

WebAssembly bindings for `spellkave-validate`, published as
`@spellkave/validate-wasm`. See the
[package usage and build instructions](https://github.com/b4mad-technologies/sk-grimoire#wasm-npm-packages).

From the workspace root, build with:

```sh
cargo build --locked --release --target-dir target --target wasm32-unknown-unknown -p spellkave-validate-wasm
```

The raw module is written under `target/wasm32-unknown-unknown/release/`.
Use the matching `wasm-bindgen` CLI to generate JavaScript bindings.
