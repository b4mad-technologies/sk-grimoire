//! Manifest composition, patching and diff operations (ADR-0087).
//! The sibling `spellkave-authoring-wasm` crate exposes these operations to Wasm callers.

pub mod applyset;
pub mod augment;
pub mod compose;
pub mod diff_apply;
pub mod diff_apply_authority;
pub mod diff_apply_extent;
pub mod grants;
pub mod patch;

/// sk-jay0.7.174.30: same reasoning as `row_kinds` below — `ROW_KIND_STORAGE_KEY`
/// lives in `spellkave-schema` (the crate `spellkave_authoring::applyset`
/// already reads it from for row identity), re-exported here so
/// `spellkave-authoring-wasm` has one dependency to bind, not two.
pub use spellkave_schema::row_identity::{row_kind_storage_key_json, ROW_KIND_STORAGE_KEY};
/// Re-exported, not owned here (`tag-vocabulary-declaration` change, Task
/// 3.2, 2026-07-27): `spellkave-validate` also needs `ROW_KIND_NATURAL_KEY`
/// (the natural-key cross-check for a manifest's `tag_prefixes` ref
/// semantics) and this crate depends on `spellkave-validate`, not the
/// reverse — so the registry now lives in the lower-level `spellkave-schema`
/// crate both already depend on. This re-export keeps every existing
/// `spellkave_authoring::row_kinds`/`spellkave_authoring::ROW_KIND_*` call
/// site unchanged; see `spellkave_schema::row_kinds`'s own doc comment.
pub use spellkave_schema::row_kinds;

pub use augment::extra_rows_by_natural_key_json;
pub use compose::aoe::{aoe_frame_write_json, shape_params_json_call};
pub use compose::clock::clock_frame_writes_json;
pub use compose::cluster_rows::cluster_rows;
pub use compose::ids::compose_ids_json;
pub use compose::live_wiring::compose_live_wiring_json;
pub use compose::modifiers::{
    affordance_predicate_json, attacks_frame_writes_json, resource_cost_debit_frame_write_json,
};
pub use compose::outcome_graph::outcome_graph_tables_json;
pub use compose::scheduled_zone::schedule_autonomy_job_effect_definition_json;
pub use compose::single_target::single_target_frame_write_json;
pub use compose::summon::summon_effect_definition_json;
pub use compose::zone::{
    zone_marker_definition_json, zone_marker_inscribe_json, zone_pulse_frame_write_json,
};
pub use patch::patch_manifest_json;
pub use row_kinds::{
    row_kind_composite_key_json, row_kind_natural_key_json, ROW_KIND_COMPOSITE_KEY,
    ROW_KIND_NATURAL_KEY,
};

#[cfg(test)]
mod tests {
    /// Trivial round-trip proving the crate compiles and links against its
    /// declared workspace dependencies before any manifest logic lands.
    #[test]
    fn parses_and_renders_a_minimal_manifest() {
        let source = "#:schema https://spellkave.dev/schema/world-manifest-1.0.0.json\nschema_version = \"1.0.0\"\n";
        let parsed = spellkave_format::parse_manifest(source).expect("minimal manifest parses");
        let rendered =
            spellkave_format::render_manifest(&parsed.manifest).expect("minimal manifest renders");
        let reparsed =
            spellkave_format::parse_manifest(&rendered).expect("rendered manifest re-parses");
        let diagnostics = spellkave_validate::validate_manifest(&reparsed.manifest);
        assert!(
            diagnostics.is_empty(),
            "minimal manifest should validate clean, got {diagnostics:?}"
        );
    }
}
