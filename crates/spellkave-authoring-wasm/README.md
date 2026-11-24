# spellkave-authoring-wasm

WebAssembly bindings for `spellkave-authoring`.

From the workspace root, build with:

```sh
cargo build --locked --release --target-dir target --target wasm32-unknown-unknown -p spellkave-authoring-wasm
```

The raw module is written under `target/wasm32-unknown-unknown/release/`.
Use the matching `wasm-bindgen` CLI to generate JavaScript bindings.
