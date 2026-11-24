//! Thin wasm-bindgen wrapper over `spellkave-authoring` (#1968 R7, ADR-0087).
//!
//! Mirrors `spellkave-validate-wasm`'s existing shape exactly: pure
//! `spellkave-authoring` logic lives in the sibling crate, this crate only
//! wasm-binds it and applies the u64-digit-string wire convention
//! (`spellkave_schema::wire_u64`) where a bound function's JSON crosses the
//! wasm/JS boundary. `serde_json`'s `preserve_order` feature is NEVER enabled
//! here — see `spellkave-validate-wasm`'s own doc comment for why (workspace-
//! wide Cargo feature unification would silently reorder every
//! `render_manifest` TOML row).

use wasm_bindgen::prelude::*;

/// Smoke-test export for the Wasm binding.
#[wasm_bindgen]
pub fn authoring_wasm_placeholder() -> String {
    "spellkave-authoring-wasm:scaffold-ok".to_owned()
}

/// Applyset part building (ADR-0145): `sources_json` is
/// `{"root":{"path","toml"},"fragments":[{"path","toml"},...]}` with the
/// fragments in the root's include order; the result is the applyset name,
/// each part's resident rows `(kind, key, bundle_id, hash)`, duplicates (the
/// last part wins) and untrackable kinds. Hashes come from the same
/// `spellkave_schema::row_identity` function the server uses. Every value is a
/// string or a count, so no `wire_u64` rewrite is needed.
#[wasm_bindgen]
pub fn build_applyset_parts_json(sources_json: &str) -> Result<String, JsValue> {
    spellkave_authoring::applyset::build_applyset_parts_json(sources_json)
        .map_err(|err| JsValue::from_str(&err))
}

/// Reads the applyset root (ADR-0145). Returns the
/// root's applyset name, its include paths in declaration order, and the
/// bundles it declares, as `{"applyset","includes":[..],"bundles":[..]}`.
#[wasm_bindgen]
pub fn applyset_root_info_json(root_toml: &str) -> Result<String, JsValue> {
    spellkave_authoring::diff_apply::applyset_root_info_json(root_toml)
        .map_err(|err| JsValue::from_str(&err))
}

/// `sk diff`/`sk apply` step 2: classify the root's rows against the applyset
/// memory (`create|configure|move|prune|unchanged`), build the delta TOML and
/// the changed parts, run the insert-only and validator checks, and compare
/// with live exports. Input and output shapes are
/// `spellkave_authoring::diff_apply::{DiffInput, DiffOutput}`. The memory
/// generation travels as a JSON number (generations stay far below 2^53).
#[wasm_bindgen]
pub fn classify_and_build_delta_json(input_json: &str) -> Result<String, JsValue> {
    spellkave_authoring::diff_apply::classify_and_build_delta_json(input_json)
        .map_err(|err| JsValue::from_str(&err))
}

/// Row identities and semantic hashes of one manifest (`is_root`) or include
/// fragment, as `{"rows":[{kind,key,bundle_id,hash}],"untrackable":[[kind,n]]}`.
#[wasm_bindgen]
pub fn manifest_row_identities_json(toml_source: &str, is_root: bool) -> Result<String, JsValue> {
    spellkave_authoring::applyset::manifest_row_identities_json(toml_source, is_root)
        .map_err(|err| JsValue::from_str(&err))
}

/// Exact identities over the same typed live JSON the comparable-hash port uses.
#[wasm_bindgen]
pub fn live_manifest_row_identities_json(manifest_json: &str) -> Result<String, JsValue> {
    spellkave_authoring::applyset::live_manifest_row_identities_json(manifest_json)
        .map_err(|err| JsValue::from_str(&err))
}

/// The manifest row-kind -> natural-key field table (#1968 R7 slice 2,
/// ADR-0087) as a JSON object string: `{"<row_kind>": "<field>", ...}`. The
/// wasm-bound counterpart of `@spellkave/world-author`'s `row-kinds.ts`
/// `ROW_KIND_NATURAL_KEY` — `row-kinds.ts` becomes a thin wrapper that
/// `JSON.parse`s this export instead of hand-carrying the literal. No
/// u64-carrying fields here (every value is a field-name string), so no
/// `wire_u64` rewrite is needed on this export.
#[wasm_bindgen]
pub fn row_kind_natural_key_json() -> String {
    spellkave_authoring::row_kind_natural_key_json()
}

/// The manifest row-kind -> ordered composite-key fields table (finding #9,
/// srd-cc battle-test ledger, 2026-07-26) as a JSON object string:
/// `{"<row_kind>": ["<field>", ...], ...}`. Sibling export to
/// `row_kind_natural_key_json` for the `*_tag` satellite kinds, which have
/// no single-column natural key — see `ROW_KIND_COMPOSITE_KEY`'s own doc
/// comment in `spellkave-authoring` for why these are a separate table
/// rather than folded into the single-column one.
#[wasm_bindgen]
pub fn row_kind_composite_key_json() -> String {
    spellkave_authoring::row_kind_composite_key_json()
}

/// The manifest row-kind -> ordered storage-key fields table (ADR-0146) as a
/// JSON object string: `{"<row_kind>": ["<field>", ...], ...}`. Sibling
/// export to `row_kind_composite_key_json` for the wider set of kinds whose
/// server-side identity/upsert key is more than the uniform `*_tag` pair —
/// see `spellkave_schema::row_identity::ROW_KIND_STORAGE_KEY`'s own doc
/// comment for the full per-kind provenance.
#[wasm_bindgen]
pub fn row_kind_storage_key_json() -> String {
    spellkave_authoring::row_kind_storage_key_json()
}

/// The row-kind -> fields table `r_export_manifest` does not round-trip
/// (sk-jay0.7.174.15) as a JSON object string:
/// `{"<row_kind or *>": ["<field>", ...], ...}`. The TS two-file diff and the
/// `live-delta-subset` gate ignore these fields by default instead of
/// carrying their own list — see
/// `spellkave_schema::export_round_trip::EXPORT_NOT_ROUND_TRIPPED`.
#[wasm_bindgen]
pub fn export_not_round_tripped_json() -> String {
    spellkave_schema::export_round_trip::export_not_round_tripped_json()
}

/// Applies one `patch.ts`-style authoring operation to a manifest (#1968 R7,
/// ADR-0087 task section 3). `manifest_json`/`payload_json` are the
/// caller's whole-manifest and op-payload objects, JSON-encoded; the return
/// value is a JSON object of ONLY the table(s) the op mutated (see
/// `spellkave_authoring::patch::support`'s module doc). Ported incrementally,
/// one op-group per slice — an op not yet ported errors the same way an
/// unrecognized op string always has.
#[wasm_bindgen]
pub fn patch_manifest_json(
    manifest_json: &str,
    op: &str,
    payload_json: &str,
) -> Result<String, JsValue> {
    spellkave_authoring::patch_manifest_json(manifest_json, op, payload_json)
        .map_err(|err| JsValue::from_str(&err))
}

/// `compose.ts`'s `ids(b, cfg)` — the namespace-aware id derivation every
/// `ActorBehavior` shape shares (#1968 R7, ADR-0087 task 4.1).
/// `behavior_json`/`cfg_json` are the caller's `ActorBehavior`/
/// `BehaviorConfig` objects, JSON-encoded (only the `key`/`capabilityId` and
/// `capPrefix`/`actionPrefix`/`idNamespace` fields are read; extra fields are
/// ignored). Returns `{"cap":...,"niche":...,"action":...,"outcome":...,
/// "affordance":...,"success":...}`.
#[wasm_bindgen]
pub fn compose_ids_json(behavior_json: &str, cfg_json: &str) -> Result<String, JsValue> {
    spellkave_authoring::compose_ids_json(behavior_json, cfg_json)
        .map_err(|err| JsValue::from_str(&err))
}

/// The shared live-wiring emission every `ActorBehavior` shape carries
/// regardless of its own topology (#1968 R7, ADR-0087 task 4.1): the
/// caster holding the capability, the niche attached to its node, plus the
/// optional caster-entity/caster-actor/`bindToDefinition`/`defInstance`
/// rows. Returns a JSON object of only the table(s) touched — see
/// `spellkave_authoring::compose::live_wiring`'s module doc.
#[wasm_bindgen]
pub fn compose_live_wiring_json(behavior_json: &str, cfg_json: &str) -> Result<String, JsValue> {
    spellkave_authoring::compose_live_wiring_json(behavior_json, cfg_json)
        .map_err(|err| JsValue::from_str(&err))
}

/// The `single-target` `ActorBehavior` shape's one frame_write body (#1968
/// R7, ADR-0087 task 4.2) — `RemoveCondition`/`ApplyStatus`/
/// `ModifyResource`, chosen by `removeCondition`/`applyCondition` priority.
/// Returns the body value alone (not wrapped in an array) — the caller
/// assembles `frameWrites` from it.
#[wasm_bindgen]
pub fn single_target_frame_write_json(behavior_json: &str) -> Result<String, JsValue> {
    spellkave_authoring::single_target_frame_write_json(behavior_json)
        .map_err(|err| JsValue::from_str(&err))
}

/// The `aoe` `ActorBehavior` shape's one frame_write body (#1968 R7,
/// ADR-0087 task 4.3) — a `ForEachActorInRadius` sphere (or overridden
/// topological shape) wrapping one `ModifyResource`.
#[wasm_bindgen]
pub fn aoe_frame_write_json(behavior_json: &str) -> Result<String, JsValue> {
    spellkave_authoring::aoe_frame_write_json(behavior_json).map_err(|err| JsValue::from_str(&err))
}

/// `shapeParamsJson(shapeId, radius)` (#1968 R7, ADR-0087 task 4.3) —
/// the ONE place either TS caller (`compose.ts`'s aoe default,
/// `authoring-verbs.ts`'s own `shapeParamsJson`) computes area-shape params
/// formatting, closing the `shapeParamsJson` hand-mirror incident (ADR-0087
/// Context) this migration was named for. `radius_json` is the radius value,
/// JSON-encoded (a bare number).
#[wasm_bindgen]
pub fn shape_params_json(shape_id: &str, radius_json: &str) -> Result<String, JsValue> {
    spellkave_authoring::shape_params_json_call(shape_id, radius_json)
        .map_err(|err| JsValue::from_str(&err))
}

/// The `attacks` (Multiattack) cross-cutting modifier's frame_writes array
/// (#1968 R7, ADR-0087 task 4.4) — N independent self-gating
/// `ModifyResource` writes, one per `MultiAttack` entry. `attacks_json` is a
/// JSON array of `MultiAttack` objects.
#[wasm_bindgen]
pub fn attacks_frame_writes_json(attacks_json: &str) -> Result<String, JsValue> {
    spellkave_authoring::attacks_frame_writes_json(attacks_json)
        .map_err(|err| JsValue::from_str(&err))
}

/// The `resourceCost` cross-cutting modifier's trailing debit frame_write
/// (#1968 R7, ADR-0087 task 4.4) — the LAST ordinal, on the caster, after
/// every damage/heal/condition write. `resource_cost_json` is
/// `{"resource": ..., "amount": ...}`.
#[wasm_bindgen]
pub fn resource_cost_debit_frame_write_json(resource_cost_json: &str) -> Result<String, JsValue> {
    spellkave_authoring::resource_cost_debit_frame_write_json(resource_cost_json)
        .map_err(|err| JsValue::from_str(&err))
}

/// The affordance_templates row's predicate portion (#1968 R7, ADR-0087
/// task 4.4) — `{"predicate_source": ...}` or `{"predicate_bool": true}`,
/// folding in both the shape's own gate (single-target/clock only, never
/// when `attacks` is set) and `resourceCost`'s pool-availability clause.
#[wasm_bindgen]
pub fn affordance_predicate_json(behavior_json: &str) -> Result<String, JsValue> {
    spellkave_authoring::affordance_predicate_json(behavior_json)
        .map_err(|err| JsValue::from_str(&err))
}

/// The `zone`/`env-state` shapes' shared marker-def row (#1968 R7, ADR-0087
/// task 4.5) — the `spatial_state_definitions` row either shape lays
/// down (int-typed for `zone`, bool-typed for `env-state`). Also used by
/// `scheduled-zone` (task 4.6, not yet ported).
#[wasm_bindgen]
pub fn zone_marker_definition_json(
    behavior_json: &str,
    bundle_id: &str,
) -> Result<String, JsValue> {
    spellkave_authoring::zone_marker_definition_json(behavior_json, bundle_id)
        .map_err(|err| JsValue::from_str(&err))
}

/// The `zone`/`env-state` shapes' shared marker `Inscribe` frame_write
/// (#1968 R7, ADR-0087 task 4.5). Also used by `scheduled-zone` (task
/// 4.6, not yet ported).
#[wasm_bindgen]
pub fn zone_marker_inscribe_json(behavior_json: &str) -> Result<String, JsValue> {
    spellkave_authoring::zone_marker_inscribe_json(behavior_json)
        .map_err(|err| JsValue::from_str(&err))
}

/// The `zone` shape's node-local pulse frame_write (#1968 R7, ADR-0087
/// task 4.5) — a `ForEachActorInRadius(node_ball, ball_radius 0)`
/// wrapping a damage `ModifyResource` or save-gated `ApplyStatus`. Also used
/// by `scheduled-zone`'s deferred re-fired pulse (task 4.6, not yet
/// ported).
#[wasm_bindgen]
pub fn zone_pulse_frame_write_json(behavior_json: &str) -> Result<String, JsValue> {
    spellkave_authoring::zone_pulse_frame_write_json(behavior_json)
        .map_err(|err| JsValue::from_str(&err))
}

/// The `scheduled-zone` shape's `ScheduleAutonomyJob` effect_definitions row
/// (#1968 R7, ADR-0087 task 4.6). `action` is the behavior's
/// already-derived action id (from `compose_ids_json`, computed once by the
/// caller).
#[wasm_bindgen]
pub fn schedule_autonomy_job_effect_definition_json(
    behavior_json: &str,
    action: &str,
    bundle_id: &str,
) -> Result<String, JsValue> {
    spellkave_authoring::schedule_autonomy_job_effect_definition_json(
        behavior_json,
        action,
        bundle_id,
    )
    .map_err(|err| JsValue::from_str(&err))
}

/// The `summon` shape's `Spawn` effect_definitions row (#1968 R7, ADR-0087
/// task 4.7). `spawn_effect_id` is the behavior's already-derived
/// `${action}/spawn` id (computed once by the caller).
#[wasm_bindgen]
pub fn summon_effect_definition_json(
    behavior_json: &str,
    spawn_effect_id: &str,
    bundle_id: &str,
) -> Result<String, JsValue> {
    spellkave_authoring::summon_effect_definition_json(behavior_json, spawn_effect_id, bundle_id)
        .map_err(|err| JsValue::from_str(&err))
}

/// The `outcome-graph` shape's full row set (#1968 R7, ADR-0087 task
/// 4.8) — `outcome_graphs`/`outcome_templates`/`outcome_graph_nodes`/
/// `outcome_graph_edges`/`consequence_templates`/`consequence_frame_writes`,
/// keyed by table. `graph_id` is the behavior's already-derived
/// `${action}/graph` id (computed once by the caller).
#[wasm_bindgen]
pub fn outcome_graph_tables_json(
    behavior_json: &str,
    graph_id: &str,
    bundle_id: &str,
) -> Result<String, JsValue> {
    spellkave_authoring::outcome_graph_tables_json(behavior_json, graph_id, bundle_id)
        .map_err(|err| JsValue::from_str(&err))
}

/// The `clock` shape's full `frameWrites` array (#1968 R7, ADR-0087 task
/// 4.9, the death-saves shape) — every tally ordinal, then every threshold
/// ordinal, each targeting `"Actor"` alone.
#[wasm_bindgen]
pub fn clock_frame_writes_json(behavior_json: &str) -> Result<String, JsValue> {
    spellkave_authoring::clock_frame_writes_json(behavior_json)
        .map_err(|err| JsValue::from_str(&err))
}

/// The candidate (target-resolved) rows for ONE row kind whose natural key
/// is NOT already present among the workspace's own rows (#1968 R7 slice
/// 5.2, ADR-0087) — the merge-by-target "HOW" half of `authoring-verbs.ts`'s
/// `augmentForValidation`. WHICH kinds to augment, WHEN (`target.kind`
/// gating), and same-bundle filtering for the place kinds stay in
/// TypeScript (policy, not manifest semantics) — see
/// `spellkave_authoring::augment`'s module doc. `existing_json`/
/// `candidate_json` are each a JSON array of row objects.
#[wasm_bindgen]
pub fn extra_rows_by_natural_key_json(
    kind: &str,
    existing_json: &str,
    candidate_json: &str,
) -> Result<String, JsValue> {
    spellkave_authoring::extra_rows_by_natural_key_json(kind, existing_json, candidate_json)
        .map_err(|err| JsValue::from_str(&err))
}

/// The whole `clusterRows(behaviors, cfg)` port (#1968 R7, ADR-0087 task
/// 4.10) — every `ActorBehavior` shape/modifier, orchestrated in Rust.
/// `behaviors_json` is a JSON ARRAY of `ActorBehavior` objects; `cfg_json` is
/// one `BehaviorConfig` object. Returns the merged, non-empty-tables-only
/// rows object.
#[wasm_bindgen]
pub fn cluster_rows_json(behaviors_json: &str, cfg_json: &str) -> Result<String, JsValue> {
    spellkave_authoring::cluster_rows(behaviors_json, cfg_json)
        .map_err(|err| JsValue::from_str(&err))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placeholder_returns_expected_marker() {
        assert_eq!(
            authoring_wasm_placeholder(),
            "spellkave-authoring-wasm:scaffold-ok"
        );
    }

    #[test]
    fn row_kind_natural_key_json_matches_native_table() {
        let json = row_kind_natural_key_json();
        let value: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");
        let obj = value.as_object().expect("object");
        assert_eq!(obj.len(), spellkave_authoring::ROW_KIND_NATURAL_KEY.len());
        for (kind, field) in spellkave_authoring::ROW_KIND_NATURAL_KEY {
            assert_eq!(obj.get(*kind).and_then(|v| v.as_str()), Some(*field));
        }
    }

    #[test]
    fn row_kind_composite_key_json_matches_native_table() {
        let json = row_kind_composite_key_json();
        let value: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");
        let obj = value.as_object().expect("object");
        assert_eq!(obj.len(), spellkave_authoring::ROW_KIND_COMPOSITE_KEY.len());
        for (kind, fields) in spellkave_authoring::ROW_KIND_COMPOSITE_KEY {
            let got = obj
                .get(*kind)
                .and_then(|v| v.as_array())
                .expect("composite key value should be an array");
            let got_fields: Vec<&str> = got.iter().filter_map(|v| v.as_str()).collect();
            assert_eq!(&got_fields, fields);
        }
    }

    #[test]
    fn row_kind_storage_key_json_matches_native_table() {
        let json = row_kind_storage_key_json();
        let value: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");
        let obj = value.as_object().expect("object");
        assert_eq!(obj.len(), spellkave_authoring::ROW_KIND_STORAGE_KEY.len());
        for (kind, fields) in spellkave_authoring::ROW_KIND_STORAGE_KEY {
            let got = obj
                .get(*kind)
                .and_then(|v| v.as_array())
                .expect("storage key value should be an array");
            let got_fields: Vec<&str> = got.iter().filter_map(|v| v.as_str()).collect();
            assert_eq!(&got_fields, fields);
        }
    }
}

/// Compose an injective role-binding key and row from caller-supplied facts.
#[wasm_bindgen]
pub fn compose_grant_json(json: &str) -> Result<String, JsValue> {
    spellkave_authoring::grants::compose_json(json).map_err(|e| JsValue::from_str(&e))
}
/// Render a bounded binding-only delta with an optional destination instance pin.
#[wasm_bindgen]
pub fn grant_delta_json(json: &str) -> Result<String, JsValue> {
    spellkave_authoring::grants::delta_json(json).map_err(|e| JsValue::from_str(&e))
}
