# Changelog

## 0.2.0

- Removed the Rust modules `spellkave_schema::refusal` and
  `spellkave_schema::action_economy_window`. The Spellkave server now owns
  the stable reducer refusal codes, turn-boundary tokens, and action-economy
  window helpers. Rust consumers of those modules must migrate before
  upgrading to this version.
- The `@spellkave/validate-wasm` and `@spellkave/authoring-wasm` JavaScript
  APIs are unchanged. Both npm packages take their version from the release
  tag.
