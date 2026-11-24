# Provenance

The Spellkave content-language workspace was copied from Spellkave
`origin/main` at `0e75d8aab0b27abf09d991d358af2e507bf4dfab` using
`git archive`. The workspace starts with its own root commit.

## Imported paths

- `crates/spellkave-dsl/`, `crates/spellkave-schema/`,
  `crates/spellkave-format/`, `crates/spellkave-validate/` and
  `crates/spellkave-authoring/`.
- `crates/spellkave-authoring-wasm/` and `crates/spellkave-validate-wasm/`.
- `packages/spellkave-mechanics/fixtures/` (54 files) and
  `packages/spellkave-mechanics/fixtures-baseline/` (5 files). These are
  frozen fixture data. Edits cover three files' comments, the baseline
  bundle description and the privileged-namespace bridge README.
- `openspec/changes/archive/2026-06-14-arbitration-outcome-graph-authoring/fixtures/authored-outcome-graph.toml`
  and `contested-grab.toml` in the same directory.
- `openspec/changes/archive/2026-07-07-self-reaction-resolution-flip/fixtures/legendary-resistance.toml`.
- `crates/Cargo.lock` moved to `Cargo.lock` and pruned by Cargo for the
  extracted dependency graph. The root `Cargo.toml` adapts the original
  `crates/Cargo.toml` workspace to the seven crates and their new root.
- `rust-toolchain.toml` retains the toolchain, components and target pin.
  The previous CI and runner configuration notes were removed.

The seven crate manifests declare `PolyForm-Shield-1.0.0`. The licence
body comes from the [PolyForm Project's tagged licence source](https://github.com/polyformproject/polyform-licenses/blob/1.0.0/PolyForm-Shield-1.0.0.md).
The required copyright notice follows the licence body. `NOTICE` retains
the applicable SRD attribution for test excerpts.

## Test handoff

The source commit above retains the original test bodies. These are handoff
locations for downstream owners.

| Destination | Original test file or case |
| --- | --- |
| `sk-content` | `crates/spellkave-dsl/tests/tokenize.rs`, `tokenize_round_trips_srd_cc_fixture_dsl_columns` |
| `sk-content` | `crates/spellkave-format/tests/manifest_json_round_trip.rs` for the SRD-wide round trip (including the coverage discussed in D18 H) |
| `sk-content` | `crates/spellkave-format/tests/row_identity.rs`, `fireball_rows_are_identified_by_kind_and_key` |
| `sk-content` | `crates/spellkave-validate/tests/dm_reference_locators.rs` and `crates/spellkave-validate/tests/dm_reference_metadata.rs` for the SRD and Elven Mini reference inventories |
| `sk-content` | `crates/spellkave-validate/tests/npc_documents.rs` for the Elven Mini quest catalog |
| `sk-npc-host` | `crates/spellkave-validate/tests/dm_srd.rs`, `crates/spellkave-validate/tests/dm_presentation.rs` and `crates/spellkave-validate/tests/frozen_dm_data.rs` |
| `sk-npc-host` | `crates/spellkave-validate/src/dm_document_envelope.rs`, `rehashed_presentation_still_refuses_all_privilege_fields_at_every_path` |
| `sk-server` bindings | `crates/spellkave-schema/tests/refusal_codes.rs`, `refusal_codes_contract_file_matches_the_crate`. The plain-identifier check stays here. |
| Drop | `crates/spellkave-format/src/lib.rs`, archive-based `render_round_trips_real_world_fixtures`. The separate mechanics round trip remains and requires its fixtures to exist. |
| Drop from this workspace | `crates/spellkave-format/tests/embedded_manifest_inventory.rs`: the two monorepo source inventories, server-only DSL oracle, and server and client directory checks. The bootstrap and mechanic fixture checks stay here. |

The source's 19 `CARGO_MANIFEST_DIR` uses in the five Rust crates were
checked along with the Wasm crate's additional use, compile-time includes
and runtime file reads. The 10 remaining uses resolve to local crate sources,
local test data, frozen mechanics or the three copied OpenSpec fixtures.

## Path changes

Each crate's `project.json` was removed. The two Wasm crates' generated
`pkg/` and `pkg-web/` trees were omitted. The workspace builds the Wasm
modules from source, and the Wasm crate READMEs describe that build.

New files are `.gitignore`, `.github/workflows/ci.yml`, `README.md`,
`PROVENANCE.md`, `LICENSE` and `NOTICE`. The original test paths removed
entirely are listed above. The following inventory lists each retained path
whose bytes differ from the source, including licence, test, formatting and
comment edits. Source code review removed references to internal
implementation files, historical design documents and monorepo tooling.
ADR numbers and descriptions of the public content language remain. The
validator replaces two internal-file references in diagnostic messages
with standalone explanations. Rust docs that appear as JSON Schema
descriptions were also edited.

- `crates/spellkave-authoring-wasm/Cargo.toml`
- `crates/spellkave-authoring-wasm/README.md`
- `crates/spellkave-authoring-wasm/src/lib.rs`
- `crates/spellkave-authoring/Cargo.toml`
- `crates/spellkave-authoring/src/applyset.rs`
- `crates/spellkave-authoring/src/compose/aoe.rs`
- `crates/spellkave-authoring/src/compose/clock.rs`
- `crates/spellkave-authoring/src/compose/cluster_rows.rs`
- `crates/spellkave-authoring/src/compose/ids.rs`
- `crates/spellkave-authoring/src/compose/live_wiring.rs`
- `crates/spellkave-authoring/src/compose/mod.rs`
- `crates/spellkave-authoring/src/compose/modifiers.rs`
- `crates/spellkave-authoring/src/compose/outcome_graph.rs`
- `crates/spellkave-authoring/src/compose/scheduled_zone.rs`
- `crates/spellkave-authoring/src/compose/single_target.rs`
- `crates/spellkave-authoring/src/compose/summon.rs`
- `crates/spellkave-authoring/src/compose/zone.rs`
- `crates/spellkave-authoring/src/diff_apply.rs`
- `crates/spellkave-authoring/src/lib.rs`
- `crates/spellkave-authoring/src/patch/capability_identity_ops.rs`
- `crates/spellkave-authoring/src/patch/communicate_ops.rs`
- `crates/spellkave-authoring/src/patch/mod.rs`
- `crates/spellkave-authoring/src/patch/no_bundle_id_ops.rs`
- `crates/spellkave-authoring/src/patch/place_graph_ops.rs`
- `crates/spellkave-authoring/src/patch/roll_table_ops.rs`
- `crates/spellkave-authoring/src/patch/simple_upsert_ops.rs`
- `crates/spellkave-authoring/src/patch/support.rs`
- `crates/spellkave-dsl/CLAUDE.md`
- `crates/spellkave-dsl/Cargo.toml`
- `crates/spellkave-dsl/src/eval.rs`
- `crates/spellkave-dsl/src/lib.rs`
- `crates/spellkave-dsl/tests/corpus.rs`
- `crates/spellkave-dsl/tests/peers_near_target.rs`
- `crates/spellkave-dsl/tests/reference_examples.rs`
- `crates/spellkave-dsl/tests/tokenize.rs`
- `crates/spellkave-format/Cargo.toml`
- `crates/spellkave-format/src/lib.rs`
- `crates/spellkave-format/tests/embedded_manifest_inventory.rs`
- `crates/spellkave-format/tests/genesis_fixture_parity.rs`
- `crates/spellkave-format/tests/parser_strictness.rs`
- `crates/spellkave-format/tests/row_identity.rs`
- `crates/spellkave-schema/Cargo.toml`
- `crates/spellkave-schema/src/action_economy_window.rs`
- `crates/spellkave-schema/src/core_bootstrap.rs`
- `crates/spellkave-schema/src/export_round_trip.rs`
- `crates/spellkave-schema/src/lib.rs`
- `crates/spellkave-schema/src/refusal.rs`
- `crates/spellkave-schema/src/row_identity.rs`
- `crates/spellkave-schema/src/row_kinds.rs`
- `crates/spellkave-schema/src/wire_u64.rs`
- `crates/spellkave-schema/tests/refusal_codes.rs`
- `crates/spellkave-validate-wasm/Cargo.toml`
- `crates/spellkave-validate-wasm/README.md`
- `crates/spellkave-validate-wasm/tests/multiruntime_drift.rs`
- `crates/spellkave-validate/Cargo.toml`
- `crates/spellkave-validate/src/action_economy.rs`
- `crates/spellkave-validate/src/affordance_selection.rs`
- `crates/spellkave-validate/src/affordance_template_participants.rs`
- `crates/spellkave-validate/src/autonomy_jobs.rs`
- `crates/spellkave-validate/src/bin/check_refs.rs`
- `crates/spellkave-validate/src/dm_document_envelope.rs`
- `crates/spellkave-validate/src/dm_duties.rs`
- `crates/spellkave-validate/src/equip_predicate.rs`
- `crates/spellkave-validate/src/guide.rs`
- `crates/spellkave-validate/src/lib.rs`
- `crates/spellkave-validate/src/locality.rs`
- `crates/spellkave-validate/src/metadata_attachments.rs`
- `crates/spellkave-validate/src/observation.rs`
- `crates/spellkave-validate/src/refs.rs`
- `crates/spellkave-validate/src/retractions.rs`
- `crates/spellkave-validate/src/tag_grammar.rs`
- `crates/spellkave-validate/src/trace_kinds.rs`
- `crates/spellkave-validate/src/visibility.rs`
- `crates/spellkave-validate/tests/cross_row_refs.rs`
- `crates/spellkave-validate/tests/place_node_trace_kind_closure.rs`
- `packages/spellkave-mechanics/fixtures-baseline/manifest.toml`
- `packages/spellkave-mechanics/fixtures/core/base/long-running-actions.toml`
- `packages/spellkave-mechanics/fixtures/mech_scale/manifest.toml`
- `packages/spellkave-mechanics/fixtures/npc-ceiling/base/actors.toml`
- `packages/spellkave-mechanics/fixtures/privileged_namespace_bridge/README.md`
- `rust-toolchain.toml`
