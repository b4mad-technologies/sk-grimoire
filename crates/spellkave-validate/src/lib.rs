mod canonical_document;
pub use canonical_document::canonical_document_hash;
pub use refs::place_seals::{MF_VAL_PLACE_SEAL, MF_VAL_SEAL_DM_BINDING};
pub use refs::role_bindings::{
    MF_VAL_BINDING_PRIVILEGED_ROLE, MF_VAL_BINDING_ROLE_REF, MF_VAL_BINDING_SUBJECT,
    MF_VAL_PRESENCE_IDENTITY_LABEL,
};
pub mod action_economy;
pub mod affordance_selection;
pub mod affordance_template_participants;
mod area_shapes;
pub mod autonomy_jobs;
pub mod containment_policy;
pub mod dm_acts;
pub mod dm_document_authority;
pub mod dm_document_envelope;
pub mod dm_duties;
pub mod dm_duty_references;
pub mod dm_guide;
pub mod dm_kit_references;
pub mod dm_menu_references;
pub mod dm_quest_references;
pub mod dm_reference_keys;
pub mod dm_reference_locators;
pub mod dm_reference_metadata;
pub mod dm_route_references;
pub mod dm_training_references;
pub mod dsl_macros;
pub mod equip_predicate;
mod equipped_numeric;
pub mod guide;
pub mod instance_owner;
pub mod locality;
pub mod metadata_attachments;
pub mod npc_documents;
pub mod numeric_dependencies;
pub mod observation;
pub mod refs;
pub use refs::roles::{
    merged_role_capability_ids, MF_VAL_ROLE_AGGREGATION_REF, MF_VAL_ROLE_CAPABILITY_REF,
    MF_VAL_ROLE_DUPLICATE,
};
pub mod retractions;
pub mod stratification;
pub mod tag_grammar;
pub mod trace_kinds;
pub mod visibility;

pub use action_economy::{
    MF_VAL_AUTONOMY_JOB_BOUNDARY_REF, MF_VAL_CHANNEL_DUP, MF_VAL_CHANNEL_RESETS_ON,
    MF_VAL_FIRE_BOUNDARY_REF, MF_VAL_TURN_SCHEDULER_ACTIVE, MF_VAL_TURN_SCHEDULER_KIND,
    MF_VAL_UNFIREABLE_BOUNDARY,
};

pub use autonomy_jobs::{
    MF_VAL_AUTONOMY_JOB_DSL_FUNCTION, MF_VAL_AUTONOMY_JOB_ID,
    MF_VAL_AUTONOMY_JOB_INELIGIBLE_EFFECT, MF_VAL_AUTONOMY_JOB_RUNTIME_BUNDLE,
};

pub use dsl_macros::{
    dsl_macro_definition_index, MF_VAL_DSL_MACRO_DEFINITION_BUNDLE,
    MF_VAL_DSL_MACRO_DEFINITION_DUP, MF_VAL_DSL_MACRO_DEFINITION_ID,
    MF_VAL_DSL_MACRO_DEFINITION_PARAMETER, MF_VAL_DSL_MACRO_DEFINITION_RETURN_TYPE,
};

pub use trace_kinds::{
    MF_VAL_PLACE_NODE_TRACE_KIND, MF_VAL_TRACE_KIND_DUP, MF_VAL_TRACE_KIND_ID,
    MF_VAL_TRACE_KIND_LOCATION_KIND,
};

pub use retractions::{MF_VAL_RETRACTION_KEY_EMPTY, MF_VAL_RETRACTION_ROW_KIND};

pub use tag_grammar::{
    MF_VAL_TAG_ALLOWED_CHARACTER, MF_VAL_TAG_FLAT, MF_VAL_TAG_PREFIX_CORE_RESERVED,
    MF_VAL_TAG_REF_AMBIGUOUS, MF_VAL_TAG_UNDECLARED_PREFIX, MF_VAL_TAG_VALUE_DEFINITION_DUP,
    MF_VAL_TAG_VALUE_DEFINITION_EMPTY, MF_VAL_TAG_VALUE_DEFINITION_OWNERSHIP,
    MF_VAL_TAG_VALUE_DEFINITION_PREFIX,
};

pub use observation::{
    MF_VAL_OBSERVATION_DESCRIPTION_LENGTH, MF_VAL_OBSERVATION_PLACEHOLDER,
    OBSERVATION_DESCRIPTION_MAX_CHARS, OBSERVATION_PLACEHOLDER_TOKENS,
};

pub use metadata_attachments::{
    METADATA_AUDIENCE_RELATIONS, METADATA_TARGET_KINDS, MF_VAL_METADATA_AUDIENCE_SHAPE,
    MF_VAL_METADATA_TARGET_KIND,
};
pub use visibility::{
    ELEVATION_CLASS_TOKENS, MF_VAL_ELEVATION_CLASS, MF_VAL_VISIBILITY_CLASS,
    VISIBILITY_CLASS_TOKENS,
};

pub use affordance_selection::MF_VAL_DUPLICATE_NICHE_OUTCOME_TEMPLATE;

pub use refs::privileged_namespace::{
    bootstrap_capability_ids, MF_VAL_ACTOR_CAPABILITY_REF, MF_VAL_FORGED_BINDING_SOURCE,
    MF_VAL_PRIVILEGED_CAPABILITY_PLACEMENT, MF_VAL_PRIVILEGED_TEMPLATE_ESCALATION,
    MF_VAL_SPAWN_DEFINITION_MODE, PRIVILEGED_CAPABILITY_PREFIX,
};
pub use refs::{
    LiveVocabulary, MF_VAL_ACTION_REQUIRED_CAPABILITY_REF, MF_VAL_CAPABILITY_DEFINITION_DUP,
    MF_VAL_SPELL_CAPABILITY_CATALOG_CONVENTION,
};
pub use refs::{
    MF_VAL_ACTOR_EQUIP_ITEM_DEF_REF, MF_VAL_ACTOR_EQUIP_SLOT_DEF_REF,
    MF_VAL_AFFORDANCE_BINDING_COVERAGE, MF_VAL_AFFORDANCE_KNOWS_ENTITY_BINDING_REF,
    MF_VAL_COMMUNICATION_SPEECH_ACT_DESCRIPTOR, MF_VAL_CONDITION_CAPABILITY_REF,
    MF_VAL_CONDITION_IMPLIES_REF, MF_VAL_CROSS_BUNDLE_SCOPE, MF_VAL_DEF_SEED_CAPABILITY_REF,
    MF_VAL_DEF_SEED_CONDITION_REF, MF_VAL_DEF_SEED_ENTITY_REF,
    MF_VAL_DEF_SEED_MODIFIER_TEMPLATE_REF, MF_VAL_DEF_SEED_STATE_REF,
    MF_VAL_DSL_EQUIPMENT_SLOT_REF, MF_VAL_DSL_FORMULA_STAT_REF, MF_VAL_DSL_ROLL_TABLE_REF,
    MF_VAL_DSL_TEMPLATE_ID_LITERAL, MF_VAL_ENTITY_DEFINITION_EQUIP_HOOK_REF,
    MF_VAL_ENTITY_TAG_ENTITY_REF, MF_VAL_EQUIP_SLOT_DUP, MF_VAL_FRAME_WRITE_CHANNEL_REF,
    MF_VAL_FRAME_WRITE_DSL_PARSE, MF_VAL_FRAME_WRITE_DSL_TYPE, MF_VAL_FRAME_WRITE_ORDINAL_DUP,
    MF_VAL_FRAME_WRITE_RESOURCE_REF, MF_VAL_HOOK_GRAFTED_REF, MF_VAL_ITEM_DEF_SLOT_REF,
    MF_VAL_MANIP_ACTION_ACTION_REF, MF_VAL_MANIP_ACTION_ON_SUCCESS, MF_VAL_MANIP_ACTION_TAG_SHAPE,
    MF_VAL_MANIP_ACTION_TAG_TYPO, MF_VAL_MANIP_ACTION_TEMPLATE_ACTION_REF,
    MF_VAL_OUTCOME_GRAPH_BOUNDS, MF_VAL_OUTCOME_GRAPH_EDGE_REF, MF_VAL_OUTCOME_GRAPH_GUARD_PARSE,
    MF_VAL_OUTCOME_GRAPH_NODE_REF, MF_VAL_OUTCOME_GRAPH_ROOT_REF, MF_VAL_ROLL_TABLE_DICE_SMUGGLING,
    MF_VAL_ROLL_TABLE_ENTRY_RESULT_REF, MF_VAL_SELF_REACTION_RESOURCE_REF,
    MF_VAL_SENSE_QUERY_KIND_DESCRIPTOR, MF_VAL_TYPED_DESCRIPTOR_LEGACY_TAG,
    MF_VAL_WEAPON_DAMAGE_SLOT_CONVENTION,
};
// ADR-0101 D-1/D-5 (movement-defaults-modes-terrain-cost).
pub use refs::{
    MF_VAL_MOVEMENT_DEFAULT_BINDING_COVERAGE, MF_VAL_MOVEMENT_MODE_BINDING_CONFLICT,
    MF_VAL_MOVEMENT_MODE_TEMPLATE_REPEATED, MF_VAL_PLACE_NICHE_ATTACHMENT,
};

use locality::LocalityClass;
use serde::{Deserialize, Serialize};
use spellkave_dsl::{compile_predicate_with_macros, validate_event_binding_refs_for_event};
use spellkave_schema::{Manifest, MetadataValue};
use std::collections::{BTreeMap, BTreeSet};

pub const MF_VAL_UNSUPPORTED_SCHEMA_VERSION: &str = "MF-VAL-001";
pub const MF_FMT_MISSING_SCHEMA_DIRECTIVE: &str = "MF-FMT-001";
pub const MF_VAL_FK_CLOSURE: &str = "MF-VAL-101";
pub const MF_VAL_SUBSTRATE_CROSSING_ANCHOR: &str = "MF-VAL-102";
pub const MF_VAL_POSE_CHECK: &str = "MF-VAL-103";
pub const MF_VAL_NODE_ID_FORMAT: &str = "MF-VAL-104";
pub const MF_VAL_EDGE_ENDPOINT: &str = "MF-VAL-105";
pub const MF_VAL_BUNDLE_DAG: &str = "MF-VAL-106";
pub const MF_VAL_H3_ANCESTRY: &str = "MF-VAL-107";
pub const MF_VAL_LOCATION_PARENT_DAG: &str = "MF-VAL-108";
/// A legacy validator-control metadata key was given a structured value.
pub const MF_VAL_METADATA_LEGACY_VALUE: &str = "MF-VAL-362";
pub const MF_VAL_PREDICATE_PARSE: &str = "MF-VAL-201";
/// Stochastic action missing resolution_predicate_source or a branch consequence.
pub const MF_VAL_STOCHASTIC_RESOLUTION_SHAPE: &str = "MF-VAL-348";
/// Stochastic action whose affordance and resolution predicates both roll.
pub const MF_VAL_STOCHASTIC_DOUBLE_ROLL: &str = "MF-VAL-349";
pub const MF_VAL_GLOBAL_STORM_UNANNOTATED: &str = "MF-VAL-202";
pub const MF_VAL_PREDICATE_CYCLE: &str = "MF-VAL-203";
pub const MF_VAL_DSL_PARSE: &str = "MF-VAL-204";
pub const MF_VAL_TARGET_KIND: &str = "MF-VAL-205";
pub const MF_VAL_EVENT_KIND: &str = "MF-VAL-206";
pub const MF_VAL_EVENT_DETAIL_FIELD: &str = "MF-VAL-207";
pub const MF_VAL_HOOK_REACHABILITY: &str = "MF-VAL-208";
pub const MF_VAL_HOOK_AUTHORING_SHAPE: &str = "MF-VAL-209";
pub const MF_VAL_STACKABLE_QUANTITY: &str = "MF-VAL-339";
pub const MF_VAL_AFFORDANCE_RANGE: &str = "MF-VAL-346";
pub const MF_VAL_REACHABILITY_PARITY: &str = "MF-VAL-347";
pub const MF_VAL_LONG_ACTION_PROMOTION: &str = "MF-VAL-302";
/// Item DSL-source slot failed a macro-aware int-expression parse (#2218).
pub const MF_VAL_ITEM_DSL_SOURCE_PARSE: &str = "MF-VAL-318";
/// Node-entry action bindings must be paired and node-anchored.
pub const MF_VAL_SPATIAL_ENTRY_ACTION_SHAPE: &str = "MF-VAL-308";
/// A non-steering author override is meaningless or internally contradictory.
pub const MF_VAL_ACTION_NON_STEERING_SHAPE: &str = "MF-VAL-309";
pub use area_shapes::MF_VAL_AREA_SHAPE_PARAMS;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Info,
    Warning,
    Fatal,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Span {
    pub start_line: usize,
    pub start_col: usize,
    pub end_line: usize,
    pub end_col: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    pub code: String,
    pub severity: Severity,
    pub message: String,
    pub path: String,
    pub span: Option<Span>,
    pub hint: Option<String>,
    pub details: Option<serde_json::Value>,
}

pub fn canonicalize_diagnostics(diags: &mut [Diagnostic]) {
    diags.sort_by(|a, b| {
        (
            a.code.as_str(),
            &a.severity,
            a.path.as_str(),
            a.span.as_ref().map(|s| s.start_line).unwrap_or(usize::MAX),
            a.span.as_ref().map(|s| s.start_col).unwrap_or(usize::MAX),
            a.message.as_str(),
        )
            .cmp(&(
                b.code.as_str(),
                &b.severity,
                b.path.as_str(),
                b.span.as_ref().map(|s| s.start_line).unwrap_or(usize::MAX),
                b.span.as_ref().map(|s| s.start_col).unwrap_or(usize::MAX),
                b.message.as_str(),
            ))
    });
}

pub fn validate_manifest_version(version: &str) -> Vec<Diagnostic> {
    if version == "1.0.0" {
        return Vec::new();
    }
    vec![Diagnostic {
        code: MF_VAL_UNSUPPORTED_SCHEMA_VERSION.to_owned(),
        severity: Severity::Fatal,
        message: "unsupported schema version".to_owned(),
        path: "schema_version".to_owned(),
        span: None,
        hint: Some("use 1.0.0".to_owned()),
        details: Some(serde_json::json!({ "received": version })),
    }]
}

pub fn validate_manifest(manifest: &Manifest) -> Vec<Diagnostic> {
    validate_manifest_with_existing_nodes(manifest, &BTreeSet::new())
}

/// Like [`validate_manifest`], but resolves `place_edges` endpoints against
/// `existing_node_ids` (already-committed `place_node` ids) in addition to the
/// manifest's own `place_nodes` (#1666). The set feeds ONLY the edge-endpoint
/// check (MF-VAL-105); it deliberately does not widen bundle/graph FK closure —
/// injecting foreign rows into those checks cascades false-fatals (see the
/// server's `augment_for_validation` doc comment). Offline callers (CLI
/// merge-gate, wasm bridge) have no live world and keep the closed-world
/// behavior via [`validate_manifest`].
pub fn validate_manifest_with_existing_nodes(
    manifest: &Manifest,
    existing_node_ids: &BTreeSet<String>,
) -> Vec<Diagnostic> {
    validate_manifest_with_world(manifest, existing_node_ids, None)
}

/// Like [`validate_manifest_with_existing_nodes`], additionally given
/// `live` — sk-pm7's existence-only live vocabulary
/// (`refs::LiveVocabulary`). `None` for every offline caller (identical to
/// [`validate_manifest_with_existing_nodes`]). An authoritative live-world
/// view can contain zero rows. The presence of `live`
/// flips the Class-6 frame-write checks (`MF_VAL_FRAME_WRITE_RESOURCE_REF` /
/// `MF_VAL_FRAME_WRITE_CHANNEL_REF`) from sk-zqc's per-file closure-
/// completeness leniency to sk-pm7's live-admission strictness — see
/// `refs::check_frame_write_ref`'s doc comment for why the two cannot share
/// one code path. The existence-only ids/bundle pairs also resolve the
/// sibling `entity_definition_equip_modifier.modifier_template_id` check
/// (MF-VAL-310) regardless of `live_world` strictness, since that check
/// never had closure-completeness leniency to begin with.
pub fn validate_manifest_with_world(
    manifest: &Manifest,
    existing_node_ids: &BTreeSet<String>,
    live: Option<&refs::LiveVocabulary>,
) -> Vec<Diagnostic> {
    validate_manifest_with_world_for_apply(manifest, manifest, existing_node_ids, live)
}

/// Validate references against `manifest`, but instance ownership against the
/// original `applying_manifest`. Live reference augmentation must not turn a
/// dependency header into an applying owner (ownership-foundation Q1).
/// Offline callers use [`validate_manifest_with_world`] with a single document.
pub fn validate_manifest_with_world_for_apply(
    manifest: &Manifest,
    applying_manifest: &Manifest,
    existing_node_ids: &BTreeSet<String>,
    live: Option<&refs::LiveVocabulary>,
) -> Vec<Diagnostic> {
    validate_manifest_impl(manifest, applying_manifest, existing_node_ids, live, live)
}

/// Author-target preflight: containment references use the same existence-only
/// vocabulary as server admission, without reconstructing dependency DSL rows.
/// Other validators retain their established offline augmentation contract.
pub fn validate_manifest_with_containment_context(
    manifest: &Manifest,
    context: &refs::LiveVocabulary,
) -> Vec<Diagnostic> {
    validate_manifest_impl(manifest, manifest, &BTreeSet::new(), None, Some(context))
}

fn validate_manifest_impl(
    manifest: &Manifest,
    applying_manifest: &Manifest,
    existing_node_ids: &BTreeSet<String>,
    live: Option<&refs::LiveVocabulary>,
    containment_context: Option<&refs::LiveVocabulary>,
) -> Vec<Diagnostic> {
    let mut diags = Vec::new();
    let metadata = &manifest.metadata;

    validate_typed_manifest_rows(manifest, existing_node_ids, live, &mut diags);
    instance_owner::validate(applying_manifest, &mut diags);
    containment_policy::validate(manifest, applying_manifest, containment_context, &mut diags);
    refs::place_seals::validate(applying_manifest, live, &mut diags);
    area_shapes::validate_area_shape_params(manifest, &mut diags);
    dm_acts::validate_for_apply(manifest, applying_manifest, live, &mut diags);
    dsl_macros::validate_dsl_macro_definitions(manifest, &mut diags);
    validate_affordance_range_m(manifest, &mut diags);
    validate_reachability_parity(manifest, &mut diags);
    validate_long_action_promotion(manifest, &mut diags);
    validate_stochastic_action_resolution(manifest, &mut diags);
    validate_action_non_steering_shape(manifest, &mut diags);
    validate_spatial_entry_action_shape(manifest, &mut diags);
    diags.extend(validate_manifest_metadata(metadata));
    validate_condition_authoring(manifest, &mut diags);
    validate_item_dsl_sources(manifest, &mut diags);
    equipped_numeric::validate(manifest, &mut diags);
    validate_affordance_predicate_locality(manifest, &mut diags);
    validate_affordance_predicate_stratification(manifest, &mut diags);
    validate_prompt_predicate_locality(manifest, &mut diags);
    validate_prompt_predicate_stratification(manifest, &mut diags);
    refs::validate_cross_row_refs_for_apply(manifest, applying_manifest, live, &mut diags);
    affordance_template_participants::validate_affordance_template_participants(
        manifest, live, &mut diags,
    );
    action_economy::validate_action_economy(manifest, &mut diags);
    affordance_selection::validate_affordance_selection_ambiguity(manifest, &mut diags);
    affordance_selection::validate_stochastic_affordance_overlap(manifest, &mut diags);
    affordance_selection::validate_duplicate_niche_outcome_templates(manifest, &mut diags);
    autonomy_jobs::validate_autonomy_jobs(manifest, &mut diags);
    trace_kinds::validate_trace_kind_definitions(manifest, &mut diags);
    trace_kinds::validate_place_node_trace_kind_closure(manifest, &mut diags);
    retractions::validate_retractions(manifest, &mut diags);
    observation::validate_observation_placeholders(manifest, &mut diags);
    tag_grammar::validate_tag_vocabulary(manifest, &mut diags);
    metadata_attachments::validate_metadata_attachment_target_kinds(manifest, &mut diags);
    metadata_attachments::validate_metadata_namespace_audiences(manifest, &mut diags);
    visibility::validate_visibility_tokens(manifest, &mut diags);

    canonicalize_diagnostics(&mut diags);
    diags
}

/// Validate the open metadata map independently of manifest rows.
///
/// Include loaders use this before discarding fragment-local authoring
/// metadata, so malformed legacy control values cannot disappear during row
/// flattening.
pub fn validate_manifest_metadata(metadata: &BTreeMap<String, MetadataValue>) -> Vec<Diagnostic> {
    let mut diags = Vec::new();
    validate_legacy_metadata_value_types(metadata, &mut diags);
    validate_fk_closure(metadata, &mut diags);
    validate_substrate_crossings(metadata, &mut diags);
    validate_pose_rows(metadata, &mut diags);
    validate_node_id_format(metadata, &mut diags);
    validate_edge_endpoints(metadata, &mut diags);
    validate_bundle_dag(metadata, &mut diags);
    validate_h3_ancestry(metadata, &mut diags);
    validate_location_parent_dag(metadata, &mut diags);
    canonicalize_diagnostics(&mut diags);
    diags
}

fn validate_action_non_steering_shape(manifest: &Manifest, diags: &mut Vec<Diagnostic>) {
    for row in &manifest.action_definitions {
        if !row.non_steering {
            continue;
        }
        if row.on_success.trim().is_empty() {
            diags.push(fatal(
                MF_VAL_ACTION_NON_STEERING_SHAPE,
                "action_definitions.non_steering",
                format!(
                    "action_definition '{}' marks non_steering=true but has no on_success consequence to classify",
                    row.action_id
                ),
            ));
        }
        if row.self_only {
            diags.push(fatal(
                MF_VAL_ACTION_NON_STEERING_SHAPE,
                "action_definitions.non_steering",
                format!(
                    "action_definition '{}' is self_only and cannot need a non-steering target-claim override",
                    row.action_id
                ),
            ));
        }
    }
}

fn validate_spatial_entry_action_shape(manifest: &Manifest, diags: &mut Vec<Diagnostic>) {
    for row in &manifest.spatial_niche_attachments {
        let has_action = !row.on_entry_action_id.trim().is_empty();
        let has_target = !row.on_entry_target_entity_key.trim().is_empty();
        if has_action != has_target {
            diags.push(fatal(
                MF_VAL_SPATIAL_ENTRY_ACTION_SHAPE,
                "spatial_niche_attachments.on_entry_action_id",
                format!(
                    "spatial_niche_attachment '{}' must author on_entry_action_id and \
                     on_entry_target_entity_key together",
                    row.niche_id
                ),
            ));
            continue;
        }
        if has_action && (row.place_node_id.trim().is_empty() || row.place_edge_id.is_some()) {
            diags.push(fatal(
                MF_VAL_SPATIAL_ENTRY_ACTION_SHAPE,
                "spatial_niche_attachments.place_node_id",
                format!(
                    "spatial_niche_attachment '{}' entry action must be anchored to exactly one \
                     place_node_id, never place_edge_id",
                    row.niche_id
                ),
            ));
        }
    }
}

fn validate_long_action_promotion(manifest: &Manifest, diags: &mut Vec<Diagnostic>) {
    for row in &manifest.action_definitions {
        if row.cost_time_ms <= 5_000 {
            continue;
        }
        let promotion_target = if row.session_def_id.is_empty() {
            "core/long_running_action"
        } else {
            row.session_def_id.as_str()
        };
        diags.push(Diagnostic {
            code: MF_VAL_LONG_ACTION_PROMOTION.into(),
            severity: Severity::Warning,
            message: format!(
                "action_definition '{}' costs {} ms and promotes to session '{}'",
                row.action_id, row.cost_time_ms, promotion_target
            ),
            path: "action_definitions.cost_time_ms".into(),
            span: None,
            hint: Some(
                "confirm the action should become busy, interruptible, and tick-driven; narrate flavor-only durations in descriptive text"
                    .into(),
            ),
            details: Some(serde_json::json!({
                "action_id": row.action_id,
                "cost_time_ms": row.cost_time_ms,
                "promotion_target": promotion_target,
                "threshold_ms": 5_000,
            })),
        });
    }
}

/// Classify predicate locality for every affordance_template row.
///
/// Also checks that Global-class predicates carry `accepts_global_storm = true`.
/// Emits MF-VAL-201 (parse failure) or MF-VAL-202 (unannotated Global).
///
/// Note: this function does NOT mutate the manifest rows — callers that need
/// the verdict for runtime dispatch should call `classify_affordance_templates`.
fn validate_stochastic_action_resolution(manifest: &Manifest, diags: &mut Vec<Diagnostic>) {
    use spellkave_schema::ActionResolutionModeRow;

    let macros = dsl_macros::dsl_macro_compiler_definitions(manifest);
    let affordances_by_action = stochastic_affordances_by_action(manifest, &macros);

    for row in &manifest.action_definitions {
        if row.resolution_mode != ActionResolutionModeRow::Stochastic {
            // Still type-check a non-empty resolution predicate if authored.
            validate_predicate_source(
                &row.resolution_predicate_source,
                "action_definitions.resolution_predicate_source",
                "action_definition",
                &row.action_id,
                &macros,
                diags,
            );
            continue;
        }

        if row.resolution_predicate_source.trim().is_empty() {
            diags.push(Diagnostic {
                code: MF_VAL_STOCHASTIC_RESOLUTION_SHAPE.to_owned(),
                severity: Severity::Fatal,
                message: format!(
                    "action_definition '{}': Stochastic resolution_mode requires a non-empty resolution_predicate_source",
                    row.action_id
                ),
                path: format!(
                    "action_definitions[{}].resolution_predicate_source",
                    row.action_id
                ),
                span: None,
                hint: Some(
                    "author a boolean DSL expression, e.g. d20 >= target.stat(\"trap/save_dc\")".into(),
                ),
                details: None,
            });
        } else {
            validate_predicate_source(
                &row.resolution_predicate_source,
                "action_definitions.resolution_predicate_source",
                "action_definition",
                &row.action_id,
                &macros,
                diags,
            );
        }

        if row.on_success.trim().is_empty() {
            diags.push(Diagnostic {
                code: MF_VAL_STOCHASTIC_RESOLUTION_SHAPE.to_owned(),
                severity: Severity::Fatal,
                message: format!(
                    "action_definition '{}': Stochastic resolution_mode requires on_success",
                    row.action_id
                ),
                path: format!("action_definitions[{}].on_success", row.action_id),
                span: None,
                hint: None,
                details: None,
            });
        }
        if row.on_failure.trim().is_empty() {
            diags.push(Diagnostic {
                code: MF_VAL_STOCHASTIC_RESOLUTION_SHAPE.to_owned(),
                severity: Severity::Fatal,
                message: format!(
                    "action_definition '{}': Stochastic resolution_mode requires on_failure",
                    row.action_id
                ),
                path: format!("action_definitions[{}].on_failure", row.action_id),
                span: None,
                hint: None,
                details: None,
            });
        }

        // Double-roll diagnostic (lint): stochastic nodes in BOTH the linked
        // affordance predicate and the resolution predicate.
        let resolution_stochastic =
            predicate_has_stochastic_ops(&row.resolution_predicate_source, &macros);
        if resolution_stochastic {
            if let Some(affordance_ids) = affordances_by_action.get(&row.action_id) {
                for (template_id, affordance_stochastic) in affordance_ids {
                    if *affordance_stochastic {
                        diags.push(Diagnostic {
                            code: MF_VAL_STOCHASTIC_DOUBLE_ROLL.to_owned(),
                            severity: Severity::Warning,
                            message: format!(
                                "action_definition '{}' and affordance_template '{}' both contain stochastic DSL nodes — prefer one stochastic owner (resolution_predicate_source)",
                                row.action_id, template_id
                            ),
                            path: format!(
                                "action_definitions[{}].resolution_predicate_source",
                                row.action_id
                            ),
                            span: None,
                            hint: Some(
                                "move the d20/roll into resolution_predicate_source and keep the affordance predicate deterministic".into(),
                            ),
                            details: Some(serde_json::json!({
                                "action_id": row.action_id,
                                "affordance_template_id": template_id,
                            })),
                        });
                    }
                }
            }
        }
    }
}

fn predicate_has_stochastic_ops(
    source: &str,
    macros: &[spellkave_dsl::DslMacroDefinition],
) -> bool {
    if source.trim().is_empty() {
        return false;
    }
    let Ok(expr) = dsl_macros::parse_predicate(source, macros) else {
        return false;
    };
    expr.nodes.iter().any(|node| {
        matches!(
            node.op,
            spellkave_dsl::Op::Roll(_) | spellkave_dsl::Op::RollTable(_)
        )
    })
}

fn stochastic_affordances_by_action(
    manifest: &Manifest,
    macros: &[spellkave_dsl::DslMacroDefinition],
) -> std::collections::BTreeMap<String, Vec<(String, bool)>> {
    use std::collections::BTreeMap;

    let outcome_to_action: BTreeMap<&str, &str> = manifest
        .outcome_templates
        .iter()
        .filter_map(|row| {
            let action = row.manipulation_action_id.as_str();
            if action.is_empty() {
                None
            } else {
                Some((row.outcome_template_id.as_str(), action))
            }
        })
        .collect();

    let mut out: BTreeMap<String, Vec<(String, bool)>> = BTreeMap::new();
    for affordance in &manifest.affordance_templates {
        let Some(action_id) = outcome_to_action.get(affordance.outcome_template_id.as_str()) else {
            continue;
        };
        let stochastic = predicate_has_stochastic_ops(&affordance.predicate_source, macros);
        out.entry((*action_id).to_string())
            .or_default()
            .push((affordance.template_id.clone(), stochastic));
    }
    out
}

pub fn validate_affordance_predicate_locality(manifest: &Manifest, diags: &mut Vec<Diagnostic>) {
    let macros = dsl_macros::dsl_macro_compiler_definitions(manifest);
    for row in &manifest.affordance_templates {
        let source = if row.predicate_source.is_empty() {
            // Empty source → boolean literal predicate (true/false).
            // Literal predicates have no subject reads → Local.
            continue;
        } else {
            row.predicate_source.as_str()
        };

        let expr = match compile_predicate_with_macros(source, &macros) {
            Ok(e) => e,
            Err(e) => {
                let msg = e
                    .diagnostics
                    .first()
                    .map(|d| d.message.clone())
                    .unwrap_or_else(|| "parse error".into());
                diags.push(Diagnostic {
                    code: MF_VAL_PREDICATE_PARSE.to_owned(),
                    severity: Severity::Fatal,
                    message: format!(
                        "affordance_template '{}': predicate parse error: {}",
                        row.template_id, msg
                    ),
                    path: "affordance_templates.predicate_source".to_owned(),
                    span: None,
                    hint: None,
                    details: Some(serde_json::json!({
                        "template_id": row.template_id,
                    })),
                });
                continue;
            }
        };

        let class = locality::classify(&expr);

        if class == LocalityClass::Global && !row.accepts_global_storm {
            diags.push(Diagnostic {
                code: MF_VAL_GLOBAL_STORM_UNANNOTATED.to_owned(),
                severity: Severity::Fatal,
                message: format!(
                    "affordance_template '{}': predicate classifies as Global but \
                     'accepts_global_storm' is not set; add 'accepts_global_storm = true' \
                     to acknowledge the storm risk",
                    row.template_id
                ),
                path: "affordance_templates.accepts_global_storm".to_owned(),
                span: None,
                hint: Some("set accepts_global_storm = true in the manifest row".to_owned()),
                details: Some(serde_json::json!({
                    "template_id": row.template_id,
                    "locality_class": "Global",
                    "missing_annotation": "accepts_global_storm",
                })),
            });
        }
    }
}

/// Run predicate-dependency graph stratification over all affordance_template rows.
///
/// Rejects any manifest where a predicate reads from the `affordance` table,
/// which would create a materialiser loop (ADR-0027 D-2).
/// Emits MF-VAL-203 (predicate cycle) for forbidden SCCs.
pub fn validate_affordance_predicate_stratification(
    manifest: &Manifest,
    diags: &mut Vec<Diagnostic>,
) {
    use stratification::{
        check_stratification, tables_read_by_predicate, MaterialiserWriteEdge, PredicateReadEdge,
    };

    let mut read_edges: Vec<PredicateReadEdge> = Vec::new();
    let mut write_edges: Vec<MaterialiserWriteEdge> = Vec::new();
    let macros = dsl_macros::dsl_macro_compiler_definitions(manifest);

    for row in &manifest.affordance_templates {
        let source = row.predicate_source.as_str();
        if source.is_empty() {
            continue;
        }
        let Ok(expr) = compile_predicate_with_macros(source, &macros) else {
            continue;
        };
        let tables = tables_read_by_predicate(&expr);
        for table in tables {
            read_edges.push(PredicateReadEdge {
                predicate_id: row.template_id.clone(),
                table_name: table,
            });
        }
        // Each template has a corresponding materialiser rule that writes `affordance`.
        write_edges.push(MaterialiserWriteEdge {
            rule_id: format!("mat/{}", row.template_id),
            target_table: stratification::AFFORDANCE_TABLE.to_owned(),
        });
    }

    let result = check_stratification(&read_edges, &write_edges, stratification::AFFORDANCE_TABLE);
    if !result.ok {
        let path = result.cycle_nodes.join(" → ");
        diags.push(Diagnostic {
            code: MF_VAL_PREDICATE_CYCLE.to_owned(),
            severity: Severity::Fatal,
            message: format!(
                "affordance_template predicates form a cycle through the 'affordance' \
                 cache table; cycle path: {}",
                path
            ),
            path: "affordance_templates.predicate_source".to_owned(),
            span: None,
            hint: Some(
                "remove reads of the 'affordance' table from affordance predicates".to_owned(),
            ),
            details: Some(serde_json::json!({
                "cycle_nodes": result.cycle_nodes,
            })),
        });
    }
}

/// Classify predicate locality for every `prompt_definition` row (ADR-0142
/// D-1), mirroring [`validate_affordance_predicate_locality`]'s ADR-0027
/// treatment of `affordance_template.predicate_source`. Emits MF-VAL-201
/// (parse failure) or MF-VAL-202 (unannotated Global).
fn validate_prompt_predicate_locality(manifest: &Manifest, diags: &mut Vec<Diagnostic>) {
    let macros = dsl_macros::dsl_macro_compiler_definitions(manifest);
    for row in &manifest.prompt_definitions {
        if row.open_predicate_source.trim().is_empty() {
            // An empty source has no authored selector to classify.
            continue;
        }

        let expr = match compile_predicate_with_macros(&row.open_predicate_source, &macros) {
            Ok(e) => e,
            Err(e) => {
                let msg = e
                    .diagnostics
                    .first()
                    .map(|d| d.message.clone())
                    .unwrap_or_else(|| "parse error".into());
                diags.push(Diagnostic {
                    code: MF_VAL_PREDICATE_PARSE.to_owned(),
                    severity: Severity::Fatal,
                    message: format!(
                        "prompt_definition '{}': open_predicate_source parse error: {}",
                        row.prompt_def_id, msg
                    ),
                    path: "prompt_definitions.open_predicate_source".to_owned(),
                    span: None,
                    hint: None,
                    details: Some(serde_json::json!({
                        "prompt_def_id": row.prompt_def_id,
                    })),
                });
                continue;
            }
        };

        let class = locality::classify(&expr);

        if class == LocalityClass::Global && !row.accepts_global_storm {
            diags.push(Diagnostic {
                code: MF_VAL_GLOBAL_STORM_UNANNOTATED.to_owned(),
                severity: Severity::Fatal,
                message: format!(
                    "prompt_definition '{}': open_predicate_source classifies as Global but \
                     'accepts_global_storm' is not set; add 'accepts_global_storm = true' \
                     to acknowledge the storm risk",
                    row.prompt_def_id
                ),
                path: "prompt_definitions.accepts_global_storm".to_owned(),
                span: None,
                hint: Some("set accepts_global_storm = true in the manifest row".to_owned()),
                details: Some(serde_json::json!({
                    "prompt_def_id": row.prompt_def_id,
                    "locality_class": "Global",
                    "missing_annotation": "accepts_global_storm",
                })),
            });
        }
    }
}

/// Run predicate-dependency graph stratification over all `prompt_definition`
/// rows (ADR-0142 D-1/D-8 gate 8), mirroring
/// [`validate_affordance_predicate_stratification`]'s ADR-0027 D-2 treatment
/// of `affordance_template`, with `prompt_occurrence`
/// (`stratification::PROMPT_OCCURRENCE_TABLE`) as the materialised
/// write-target instead of the retired `affordance` cache. Emits MF-VAL-203
/// (predicate cycle) for a forbidden SCC.
fn validate_prompt_predicate_stratification(manifest: &Manifest, diags: &mut Vec<Diagnostic>) {
    use stratification::{
        check_stratification, tables_read_by_predicate, MaterialiserWriteEdge, PredicateReadEdge,
        PROMPT_OCCURRENCE_TABLE,
    };

    let mut read_edges: Vec<PredicateReadEdge> = Vec::new();
    let mut write_edges: Vec<MaterialiserWriteEdge> = Vec::new();
    let macros = dsl_macros::dsl_macro_compiler_definitions(manifest);

    for row in &manifest.prompt_definitions {
        let source = row.open_predicate_source.as_str();
        if source.is_empty() {
            continue;
        }
        let Ok(expr) = compile_predicate_with_macros(source, &macros) else {
            continue;
        };
        let tables = tables_read_by_predicate(&expr);
        for table in tables {
            read_edges.push(PredicateReadEdge {
                predicate_id: row.prompt_def_id.clone(),
                table_name: table,
            });
        }
        // Each prompt_definition has a corresponding occurrence-lifecycle
        // reconcile that writes prompt_occurrence
        // (world_interaction::affordance_materialiser::prompt_occurrence).
        write_edges.push(MaterialiserWriteEdge {
            rule_id: format!("prompt_occurrence/{}", row.prompt_def_id),
            target_table: PROMPT_OCCURRENCE_TABLE.to_owned(),
        });
    }

    let result = check_stratification(&read_edges, &write_edges, PROMPT_OCCURRENCE_TABLE);
    if !result.ok {
        let path = result.cycle_nodes.join(" → ");
        diags.push(Diagnostic {
            code: MF_VAL_PREDICATE_CYCLE.to_owned(),
            severity: Severity::Fatal,
            message: format!(
                "prompt_definition open_predicate_source predicates form a cycle through the \
                 'prompt_occurrence' cache table; cycle path: {}",
                path
            ),
            path: "prompt_definitions.open_predicate_source".to_owned(),
            span: None,
            hint: Some(
                "remove reads of the 'prompt_occurrence' table from open_predicate_source"
                    .to_owned(),
            ),
            details: Some(serde_json::json!({
                "cycle_nodes": result.cycle_nodes,
            })),
        });
    }
}

/// Classify all affordance_template rows and return their locality verdicts.
///
/// Used by callers that need to persist the verdict (e.g. manifest apply).
/// Returns `(template_id, LocalityClass)` pairs; templates without a
/// `predicate_source` default to `Local`.
pub fn classify_affordance_templates(manifest: &Manifest) -> Vec<(String, LocalityClass)> {
    let macros = dsl_macros::dsl_macro_compiler_definitions(manifest);
    manifest
        .affordance_templates
        .iter()
        .map(|row| {
            let class = if row.predicate_source.is_empty() {
                LocalityClass::Local
            } else {
                match compile_predicate_with_macros(&row.predicate_source, &macros) {
                    Ok(expr) => locality::classify(&expr),
                    Err(_) => LocalityClass::Global,
                }
            };
            (row.template_id.clone(), class)
        })
        .collect()
}

fn validate_typed_manifest_rows(
    manifest: &Manifest,
    existing_node_ids: &BTreeSet<String>,
    live: Option<&refs::LiveVocabulary>,
    diags: &mut Vec<Diagnostic>,
) {
    // sk-jay0.7.174.23: widened with `live.bundles`/`live.place_graphs`
    // (existence-only, already-published) so a delta that redeclares one
    // `place_node`/`place_edge` without repeating its `[[bundles]]`/
    // `[[place_graphs]]` header still resolves — see `LiveVocabulary`'s doc
    // comment for why only the bare id (never a full row) is trusted here.
    // An id absent from BOTH the delta and the live set stays fatal.
    let bundles: BTreeSet<&str> = manifest
        .bundles
        .iter()
        .map(|row| row.bundle_id.as_str())
        .chain(["core"])
        .chain(
            live.into_iter()
                .flat_map(|l| l.bundles.iter().map(String::as_str)),
        )
        .collect();
    for (kind, role, bundle) in manifest
        .role_definitions
        .iter()
        .map(|r| ("role_definitions", &r.role_id, &r.bundle_id))
        .chain(
            manifest
                .role_aggregations
                .iter()
                .map(|r| ("role_aggregations", &r.role_id, &r.bundle_id)),
        )
    {
        if !bundles.contains(bundle.as_str()) {
            diags.push(fatal(
                MF_VAL_FK_CLOSURE,
                &format!("{kind}.bundle_id"),
                format!("role '{role}' references unknown owning bundle '{bundle}'"),
            ));
        }
    }
    let graphs: BTreeSet<&str> = manifest
        .place_graphs
        .iter()
        .map(|row| row.place_graph_id.as_str())
        .chain(
            live.into_iter()
                .flat_map(|l| l.place_graphs.keys().map(String::as_str)),
        )
        .collect();
    let mut extent_kinds: BTreeMap<&str, &str> = spellkave_schema::CORE_EXTENT_KINDS
        .iter()
        .map(|row| (row.kind_id, row.resolution_shape))
        .collect();
    for row in &manifest.extent_kind_definitions {
        extent_kinds.insert(row.kind_id.as_str(), row.resolution_shape.as_str());
        if !matches!(
            row.resolution_shape.as_str(),
            "PlaceGraph" | "Party" | "Bundle"
        ) {
            diags.push(fatal(
                MF_VAL_FK_CLOSURE,
                "extent_kind_definitions.resolution_shape",
                format!(
                    "extent_kind_definition '{}' has unknown resolution_shape '{}'",
                    row.kind_id, row.resolution_shape
                ),
            ));
        }
        if !bundles.contains(row.bundle_id.as_str()) {
            diags.push(fatal(
                MF_VAL_FK_CLOSURE,
                "extent_kind_definitions.bundle_id",
                format!(
                    "extent_kind_definition '{}' references unknown bundle '{}'",
                    row.kind_id, row.bundle_id
                ),
            ));
        }
    }
    // ADR-0134 D-1: `distance_metric_definitions.metric_shape` is the closed
    // kernel formula set; the bundle must be declared (same tier as extent kinds).
    for row in &manifest.distance_metric_definitions {
        if !matches!(
            row.metric_shape.as_str(),
            "Euclidean" | "Chebyshev" | "Manhattan"
        ) {
            diags.push(fatal(
                MF_VAL_FK_CLOSURE,
                "distance_metric_definitions.metric_shape",
                format!(
                    "distance_metric_definition '{}' has unknown metric_shape '{}' (expected Euclidean, Chebyshev or Manhattan)",
                    row.metric_id, row.metric_shape
                ),
            ));
        }
        if !bundles.contains(row.bundle_id.as_str()) {
            diags.push(fatal(
                MF_VAL_FK_CLOSURE,
                "distance_metric_definitions.bundle_id",
                format!(
                    "distance_metric_definition '{}' references unknown bundle '{}'",
                    row.metric_id, row.bundle_id
                ),
            ));
        }
    }
    // ADR-0142 D-4: `core/audience/directed_trace` and `core/audience/broadcast`
    // are always resolvable (seeded core rows), same posture as `place_graph`/
    // `party` above for `extent_kinds`.
    let mut audience_kinds: BTreeMap<&str, &str> = BTreeMap::from([
        ("core/audience/directed_trace", "DirectedTrace"),
        ("core/audience/broadcast", "Broadcast"),
    ]);
    for row in &manifest.audience_kind_definitions {
        audience_kinds.insert(row.audience_kind_id.as_str(), row.resolution_shape.as_str());
        if row.resolution_shape != "DirectedTrace" && row.resolution_shape != "Broadcast" {
            diags.push(fatal(
                MF_VAL_FK_CLOSURE,
                "audience_kind_definitions.resolution_shape",
                format!(
                    "audience_kind_definition '{}' has unknown resolution_shape '{}'",
                    row.audience_kind_id, row.resolution_shape
                ),
            ));
        }
        if !bundles.contains(row.bundle_id.as_str()) {
            diags.push(fatal(
                MF_VAL_FK_CLOSURE,
                "audience_kind_definitions.bundle_id",
                format!(
                    "audience_kind_definition '{}' references unknown bundle '{}'",
                    row.audience_kind_id, row.bundle_id
                ),
            ));
        }
    }
    // ADR-0142 D-1: `prompt_definition.response_action_ids` names existing
    // `action_definition` rows; `audience_kind_ref` resolves via the map
    // above. sk-jay0.2.33.15.7 (canon-warden A-4): the response_action_ids
    // EXISTENCE + BUNDLE-SCOPE check itself moved to
    // `refs::validate_prompt_response_action_refs` — it needs `RefIndex`
    // (the merged manifest-plus-live action map, plus `BundleScope`), which
    // this function does not have. Only the row-local completeness checks
    // (bundle_id, audience_kind_ref, non-empty response_action_ids) stay here.
    for row in &manifest.prompt_definitions {
        if !bundles.contains(row.bundle_id.as_str()) {
            diags.push(fatal(
                MF_VAL_FK_CLOSURE,
                "prompt_definitions.bundle_id",
                format!(
                    "prompt_definition '{}' references unknown bundle '{}'",
                    row.prompt_def_id, row.bundle_id
                ),
            ));
        }
        if !audience_kinds.contains_key(row.audience_kind_ref.as_str()) {
            diags.push(fatal(
                MF_VAL_FK_CLOSURE,
                "prompt_definitions.audience_kind_ref",
                format!(
                    "prompt_definition '{}' references unknown audience_kind_ref '{}'",
                    row.prompt_def_id, row.audience_kind_ref
                ),
            ));
        }
        if row.response_action_ids.is_empty() {
            diags.push(fatal(
                MF_VAL_FK_CLOSURE,
                "prompt_definitions.response_action_ids",
                format!(
                    "prompt_definition '{}' has no response_action_ids",
                    row.prompt_def_id
                ),
            ));
        }
    }

    let nodes: BTreeSet<&str> = manifest
        .place_nodes
        .iter()
        .map(|row| row.node_id.as_str())
        .collect();
    let locations: BTreeSet<&str> = manifest
        .location_definitions
        .iter()
        .map(|row| row.location_def_id.as_str())
        .collect();
    let modifier_templates: BTreeSet<&str> = manifest
        .modifier_templates
        .iter()
        .map(|row| row.modifier_template_id.as_str())
        .collect();
    let condition_definitions: BTreeSet<&str> = manifest
        .condition_definitions
        .iter()
        .map(|row| row.condition_definition_id.as_str())
        .collect();
    let entity_definitions: BTreeMap<&str, bool> = manifest
        .entity_definitions
        .iter()
        .map(|row| (row.entity_def_id.as_str(), row.stackable))
        .collect();
    for row in &manifest.bundles {
        for dep in &row.depends_on {
            if !bundles.contains(dep.as_str()) {
                diags.push(fatal(
                    MF_VAL_FK_CLOSURE,
                    "bundles.depends_on",
                    format!(
                        "bundle '{}' depends on unknown bundle '{}'",
                        row.bundle_id, dep
                    ),
                ));
            }
        }
    }

    for row in &manifest.place_graphs {
        if !bundles.contains(row.bundle_id.as_str()) {
            diags.push(fatal(
                MF_VAL_FK_CLOSURE,
                "place_graphs.bundle_id",
                format!(
                    "place_graph '{}' references unknown bundle '{}'",
                    row.place_graph_id, row.bundle_id
                ),
            ));
        }
    }

    for row in &manifest.place_nodes {
        if !bundles.contains(row.bundle_id.as_str()) {
            diags.push(fatal(
                MF_VAL_FK_CLOSURE,
                "place_nodes.bundle_id",
                format!(
                    "place_node '{}' references unknown bundle '{}'",
                    row.node_id, row.bundle_id
                ),
            ));
        }
        if !graphs.contains(row.place_graph_id.as_str()) {
            diags.push(fatal(
                MF_VAL_FK_CLOSURE,
                "place_nodes.place_graph_id",
                format!(
                    "place_node '{}' references unknown graph '{}'",
                    row.node_id, row.place_graph_id
                ),
            ));
        }
        if !is_valid_place_node_id(&row.node_id, &row.place_graph_id, &row.local_id) {
            diags.push(fatal(
                MF_VAL_NODE_ID_FORMAT,
                "place_nodes.node_id",
                format!("invalid place_node id '{}'", row.node_id),
            ));
        }
    }

    for row in &manifest.place_edges {
        if !bundles.contains(row.bundle_id.as_str()) {
            diags.push(fatal(
                MF_VAL_FK_CLOSURE,
                "place_edges.bundle_id",
                format!(
                    "place_edge '{}' references unknown bundle '{}'",
                    row.edge_key, row.bundle_id
                ),
            ));
        }
        if !row.place_graph_id.is_empty() && !graphs.contains(row.place_graph_id.as_str()) {
            diags.push(fatal(
                MF_VAL_FK_CLOSURE,
                "place_edges.place_graph_id",
                format!(
                    "place_edge '{}' references unknown graph '{}'",
                    row.edge_key, row.place_graph_id
                ),
            ));
        }
        let endpoint_resolves =
            |node_id: &str| nodes.contains(node_id) || existing_node_ids.contains(node_id);
        if !endpoint_resolves(row.from_node_id.as_str())
            || !endpoint_resolves(row.to_node_id.as_str())
        {
            diags.push(fatal(
                MF_VAL_EDGE_ENDPOINT,
                "place_edges",
                format!(
                    "edge endpoints must resolve: {}:{}->{}",
                    row.edge_key, row.from_node_id, row.to_node_id
                ),
            ));
        }
        if row.substrate_crossing && (!row.cell_anchor_present || row.cell_anchor == 0) {
            diags.push(fatal(
                MF_VAL_SUBSTRATE_CROSSING_ANCHOR,
                "place_edges.cell_anchor",
                format!(
                    "cross-substrate edge '{}' requires a non-zero cell anchor",
                    row.edge_key
                ),
            ));
        }
    }

    for (extent_kind_ref, extent_ref, table) in manifest
        .actor_capabilities
        .iter()
        .map(|row| {
            (
                row.extent_kind_ref.as_str(),
                row.extent_ref.as_str(),
                "actor_capabilities",
            )
        })
        .chain(
            manifest
                .entity_definition_initial_capability
                .iter()
                .map(|row| {
                    (
                        row.extent_kind_ref.as_str(),
                        row.extent_ref.as_str(),
                        "entity_definition_initial_capability",
                    )
                }),
        )
    {
        let extent_kind = if extent_kind_ref.is_empty() {
            "place_graph"
        } else {
            extent_kind_ref
        };
        let Some(resolution_shape) = extent_kinds.get(extent_kind).copied() else {
            diags.push(fatal(
                MF_VAL_FK_CLOSURE,
                &format!("{table}.extent_kind_ref"),
                format!(
                    "extent_kind_ref '{}' does not resolve to an extent_kind_definition",
                    extent_kind_ref
                ),
            ));
            continue;
        };
        if resolution_shape == "Bundle" && !extent_ref.is_empty() && !bundles.contains(extent_ref) {
            diags.push(fatal(
                MF_VAL_FK_CLOSURE,
                &format!("{table}.extent_ref"),
                format!(
                    "bundle extent '{}' does not resolve to a declared bundle",
                    extent_ref
                ),
            ));
        }
        if resolution_shape == "PlaceGraph"
            && !extent_ref.is_empty()
            && !graphs.contains(extent_ref)
        {
            diags.push(fatal(
                MF_VAL_FK_CLOSURE,
                &format!("{table}.extent_ref"),
                format!(
                    "extent_ref '{}' does not resolve to a declared place graph",
                    extent_ref
                ),
            ));
        }
    }

    for row in &manifest.location_definitions {
        if !bundles.contains(row.bundle_id.as_str()) {
            diags.push(fatal(
                MF_VAL_FK_CLOSURE,
                "location_definitions.bundle_id",
                format!(
                    "location '{}' references unknown bundle '{}'",
                    row.location_def_id, row.bundle_id
                ),
            ));
        }
        if !row.parent_def.is_empty() && !locations.contains(row.parent_def.as_str()) {
            diags.push(fatal(
                MF_VAL_FK_CLOSURE,
                "location_definitions.parent_def",
                format!(
                    "location '{}' references unknown parent '{}'",
                    row.location_def_id, row.parent_def
                ),
            ));
        }
        if !row.interior_graph_id.is_empty() && !graphs.contains(row.interior_graph_id.as_str()) {
            diags.push(fatal(
                MF_VAL_FK_CLOSURE,
                "location_definitions.interior_graph_id",
                format!(
                    "location '{}' references unknown graph '{}'",
                    row.location_def_id, row.interior_graph_id
                ),
            ));
        }
    }

    for row in &manifest.place_node_traces {
        if !nodes.contains(row.node_id.as_str()) && !existing_node_ids.contains(&row.node_id) {
            diags.push(fatal(
                MF_VAL_FK_CLOSURE,
                "place_node_traces.node_id",
                format!("place_node_trace references unknown node '{}'", row.node_id),
            ));
        }
    }

    for row in &manifest.location_traces {
        if !locations.contains(row.location_def_id.as_str()) {
            diags.push(fatal(
                MF_VAL_FK_CLOSURE,
                "location_traces.location_def_id",
                format!(
                    "location_trace references unknown location '{}'",
                    row.location_def_id
                ),
            ));
        }
    }

    for row in &manifest.condition_definitions {
        if !bundles.contains(row.bundle_id.as_str()) {
            diags.push(fatal(
                MF_VAL_FK_CLOSURE,
                "condition_definitions.bundle_id",
                format!(
                    "condition_definition '{}' references unknown bundle '{}'",
                    row.condition_definition_id, row.bundle_id
                ),
            ));
        }
        if !row.modifier_template_id.is_empty()
            && !modifier_templates.contains(row.modifier_template_id.as_str())
        {
            diags.push(fatal(
                MF_VAL_FK_CLOSURE,
                "condition_definitions.modifier_template_id",
                format!(
                    "condition_definition '{}' references unknown modifier_template '{}'",
                    row.condition_definition_id, row.modifier_template_id
                ),
            ));
        }
    }

    for row in &manifest.target_conditions {
        if !condition_definitions.contains(row.condition_definition_id.as_str()) {
            diags.push(fatal(
                MF_VAL_FK_CLOSURE,
                "target_conditions.condition_definition_id",
                format!(
                    "target_condition references unknown condition_definition '{}'",
                    row.condition_definition_id
                ),
            ));
        }
    }

    for row in &manifest.entities {
        let entity_def_id = if row.entity_def_id.is_empty() {
            format!("{}:{}", row.bundle_id, row.entity_key)
        } else {
            row.entity_def_id.clone()
        };
        if row.quantity > 1 && entity_definitions.get(entity_def_id.as_str()) != Some(&true) {
            diags.push(fatal(
                MF_VAL_STACKABLE_QUANTITY,
                "entities.quantity",
                format!(
                    "entity '{}' quantity {} > 1 requires 'stackable = true' on entity_definition '{}'",
                    row.entity_key, row.quantity, entity_def_id
                ),
            ));
        }
    }
}

fn validate_affordance_range_m(manifest: &Manifest, diags: &mut Vec<Diagnostic>) {
    for row in &manifest.affordance_templates {
        if row.range_m.is_finite() && row.range_m >= 0.0 {
            continue;
        }
        diags.push(Diagnostic {
            code: MF_VAL_AFFORDANCE_RANGE.to_owned(),
            severity: Severity::Fatal,
            message: format!(
                "affordance_template '{}' range_m must be finite and non-negative",
                row.template_id
            ),
            path: format!("affordance_templates[{}].range_m", row.template_id),
            span: None,
            hint: Some(
                "author range_m in non-negative metric units; omit it for touch range".to_owned(),
            ),
            details: Some(serde_json::json!({
                "template_id": row.template_id,
                "range_m": row.range_m,
            })),
        });
    }
}

fn validate_reachability_parity(manifest: &Manifest, diags: &mut Vec<Diagnostic>) {
    let outcomes: BTreeMap<&str, &str> = manifest
        .outcome_templates
        .iter()
        .map(|row| {
            (
                row.outcome_template_id.as_str(),
                row.manipulation_action_id.as_str(),
            )
        })
        .collect();
    for row in &manifest.affordance_templates {
        if row.range_m == 0.0 {
            continue;
        }
        if outcomes
            .get(row.outcome_template_id.as_str())
            .is_some_and(|action_id| !action_id.is_empty())
        {
            continue;
        }
        diags.push(Diagnostic {
            code: MF_VAL_REACHABILITY_PARITY.to_owned(),
            severity: Severity::Fatal,
            message: format!(
                "affordance_template '{}' range_m requires a manipulation-scoped outcome_template so perceive and act share the same reachability rule",
                row.template_id
            ),
            path: format!("affordance_templates[{}].range_m", row.template_id),
            span: None,
            hint: Some(
                "set range_m only on templates whose outcome_template carries manipulation_action_id"
                    .to_owned(),
            ),
            details: Some(serde_json::json!({
                "template_id": row.template_id,
                "outcome_template_id": row.outcome_template_id,
                "range_m": row.range_m,
                "shared_source": "affordance_template.range_m",
                "shared_rule": "metric place_node distance",
            })),
        });
    }
}

/// Accepted `modifier_template.delta_kind` values (ADR-0038).
const DELTA_KIND_LEGAL: &[&str] = &[
    "Advantage",
    "Disadvantage",
    "Plus",
    "Times",
    "FlatSet",
    "Replace",
];

/// `modifier_template.delta_kind` is not a member of the legal `ModifierDeltaTag` set.
pub const MF_VAL_MODIFIER_TEMPLATE_DELTA_KIND: &str = "MF-VAL-243";

fn validate_condition_authoring(manifest: &Manifest, diags: &mut Vec<Diagnostic>) {
    let macros = dsl_macros::dsl_macro_compiler_definitions(manifest);
    for row in &manifest.modifier_templates {
        validate_predicate_source(
            &row.predicate_source,
            "modifier_templates.predicate_source",
            "modifier_template",
            &row.modifier_template_id,
            &macros,
            diags,
        );
        validate_outcome_source(
            &row.modifier_expression_source,
            "modifier_templates.modifier_expression_source",
            "modifier_template",
            &row.modifier_template_id,
            &macros,
            diags,
        );
        // CR-3 / ADR-0038 (COND-2): close the kernel-coupled `delta_kind`
        // value-set at admit-time so a typo fails loudly here instead of
        // silently fail-soft'ing to a dropped modifier at runtime.
        if !DELTA_KIND_LEGAL.contains(&row.delta_kind.as_str()) {
            diags.push(Diagnostic {
                code: MF_VAL_MODIFIER_TEMPLATE_DELTA_KIND.to_owned(),
                severity: Severity::Fatal,
                message: format!(
                    "modifier_template '{}' delta_kind '{}' is not one of {}",
                    row.modifier_template_id,
                    row.delta_kind,
                    DELTA_KIND_LEGAL.join("|")
                ),
                path: format!(
                    "modifier_templates[{}].delta_kind",
                    row.modifier_template_id
                ),
                span: None,
                hint: Some(format!("use one of: {}", DELTA_KIND_LEGAL.join(", "))),
                details: None,
            });
        }
    }

    for row in &manifest.condition_definitions {
        validate_predicate_source(
            &row.gate_predicate_source,
            "condition_definitions.gate_predicate_source",
            "condition_definition",
            &row.condition_definition_id,
            &macros,
            diags,
        );
        let parsed_expr = if row.gate_predicate_source.is_empty() {
            None
        } else {
            dsl_macros::parse_predicate(&row.gate_predicate_source, &macros).ok()
        };
        for (hook_index, hook) in row.hooks.iter().enumerate() {
            validate_event_kind_source(
                &hook.event_kind,
                "condition_definitions.hooks.event_kind",
                &row.condition_definition_id,
                diags,
            );
            validate_hook_event_bindings(
                &row.gate_predicate_source,
                hook.event_kind.as_str(),
                "condition_definitions.hooks.detail_filter",
                &row.condition_definition_id,
                &macros,
                diags,
            );

            // Resolve the hook's authoring shape (issue #257). The two
            // accepted shapes are XOR per field:
            //   • numeric `expression` / `detail_filter` — legacy node-id
            //     offsets into the parsed gate_predicate AST.
            //   • inline DSL `expression_source` / `detail_filter_source`
            //     — parsed on its own and attached to a clone of the gate predicate.
            let Some(resolved) = resolve_hook_authoring_shape(
                &row.condition_definition_id,
                hook_index,
                hook,
                parsed_expr.as_ref(),
                &macros,
                diags,
            ) else {
                continue;
            };
            validate_hook_composer_reachability(
                &row.condition_definition_id,
                &row.modifier_template_id,
                hook_index as u32,
                hook.event_kind.as_str(),
                resolved.expression_node_id,
                &resolved.merged_expr,
                diags,
            );
        }
    }

    validate_target_conditions(manifest, diags);
}

/// One hook's resolved authoring shape: a merged gate_predicate AST plus the
/// final node id that the runtime's `hook.expression` field will reference.
struct ResolvedHookShape {
    merged_expr: spellkave_dsl::Expression,
    expression_node_id: u32,
}

/// XOR-check the authoring shape on one hook row and return the merged
/// expression + resolved `expression` node id. Emits one or more
/// [`MF_VAL_HOOK_AUTHORING_SHAPE`] diagnostics on shape conflicts; returns
/// `None` when the row is unusable for downstream reachability checks.
fn resolve_hook_authoring_shape(
    condition_definition_id: &str,
    hook_index: usize,
    hook: &spellkave_schema::ConditionHookRow,
    parsed_gate: Option<&spellkave_dsl::Expression>,
    macros: &[spellkave_dsl::DslMacroDefinition],
    diags: &mut Vec<Diagnostic>,
) -> Option<ResolvedHookShape> {
    let path_prefix = format!(
        "condition_definitions.hooks[{hook_index}] (condition_definition '{condition_definition_id}')"
    );

    // Expression: exactly one shape.
    match (&hook.expression, &hook.expression_source) {
        (Some(_), Some(_)) => {
            diags.push(fatal(
                MF_VAL_HOOK_AUTHORING_SHAPE,
                "condition_definitions.hooks.expression",
                format!(
                    "{path_prefix}: both 'expression' and 'expression_source' set; pick one (prefer 'expression_source')"
                ),
            ));
            return None;
        }
        (None, None) => {
            diags.push(fatal(
                MF_VAL_HOOK_AUTHORING_SHAPE,
                "condition_definitions.hooks.expression",
                format!(
                    "{path_prefix}: missing 'expression' or 'expression_source' (prefer 'expression_source')"
                ),
            ));
            return None;
        }
        _ => {}
    }
    if hook.detail_filter.is_some() && hook.detail_filter_source.is_some() {
        diags.push(fatal(
            MF_VAL_HOOK_AUTHORING_SHAPE,
            "condition_definitions.hooks.detail_filter",
            format!("{path_prefix}: both 'detail_filter' and 'detail_filter_source' set; pick one"),
        ));
        return None;
    }

    let parsed_gate = parsed_gate?;
    let mut merged = parsed_gate.clone();

    let expression_node_id = match (&hook.expression, &hook.expression_source) {
        (Some(node_id), None) => *node_id,
        (None, Some(source)) => match dsl_macros::parse_outcome(source, macros) {
            Ok(sub) => merged.graft(&sub),
            Err(failure) => {
                for diag in &failure.diagnostics {
                    diags.push(fatal(
                        MF_VAL_DSL_PARSE,
                        "condition_definitions.hooks.expression_source",
                        format!(
                            "{path_prefix}: expression_source parse [{}] {}",
                            diag.code, diag.message
                        ),
                    ));
                }
                return None;
            }
        },
        _ => unreachable!("XOR enforced above"),
    };

    if let Some(source) = hook.detail_filter_source.as_ref() {
        match dsl_macros::parse_predicate(source, macros) {
            Ok(sub) => {
                let _ = merged.graft(&sub);
            }
            Err(failure) => {
                for diag in &failure.diagnostics {
                    diags.push(fatal(
                        MF_VAL_DSL_PARSE,
                        "condition_definitions.hooks.detail_filter_source",
                        format!(
                            "{path_prefix}: detail_filter_source parse [{}] {}",
                            diag.code, diag.message
                        ),
                    ));
                }
                return None;
            }
        }
    }

    Some(ResolvedHookShape {
        merged_expr: merged,
        expression_node_id,
    })
}

/// AST origin for the hook reachability walk.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HookExpressionSource {
    /// The manifest validator follows both children of `RollTable`.
    Manifest,
    /// The server's stored AST walk follows only the draw child of `RollTable`.
    Server,
}

/// Intent kinds emitted by a hook without a registered composer for `event_kind`.
///
/// The caller owns event-path and expression-root validation. A modifier
/// template contributes a `ModifierDelta` when the root is absent or does
/// not itself emit a side-effect intent.
pub fn unregistered_hook_intent_kinds(
    event_kind: &str,
    modifier_template_id: &str,
    hook_expression: spellkave_dsl::NodeId,
    expr: &spellkave_dsl::Expression,
    source: HookExpressionSource,
) -> BTreeSet<&'static str> {
    use spellkave_schema::{is_composer_registered, CONDITION_HOOK_INTENT_KIND_MODIFIER_DELTA};

    let mut intent_kinds = BTreeSet::new();
    collect_hook_intent_kinds(
        expr,
        hook_expression,
        source,
        &mut intent_kinds,
        &mut BTreeSet::new(),
    );

    let root_is_side_effect = expr
        .node(hook_expression)
        .is_some_and(|root| side_effect_intent_tag(&root.op).is_some());
    if !root_is_side_effect && !modifier_template_id.is_empty() {
        intent_kinds.insert(CONDITION_HOOK_INTENT_KIND_MODIFIER_DELTA);
    }
    intent_kinds.retain(|intent_kind| !is_composer_registered(event_kind, intent_kind));
    intent_kinds
}

fn validate_hook_composer_reachability(
    condition_definition_id: &str,
    modifier_template_id: &str,
    hook_index: u32,
    event_kind: &str,
    hook_expression: u32,
    expr: &spellkave_dsl::Expression,
    diags: &mut Vec<Diagnostic>,
) {
    if parse_event_kind_path(event_kind).is_err() || expr.node(hook_expression).is_none() {
        return;
    }
    for intent_kind in unregistered_hook_intent_kinds(
        event_kind,
        modifier_template_id,
        hook_expression,
        expr,
        HookExpressionSource::Manifest,
    ) {
        diags.push(fatal(
            MF_VAL_HOOK_REACHABILITY,
            "condition_definitions.hooks.expression",
            format!(
                "condition_definition '{condition_definition_id}' hook[{hook_index}] for \
                 event_kind '{event_kind}' emits a '{intent_kind}' intent but no §5.19 composer \
                 is registered for that pair; register one in \
                 spellkave_schema::composer_registrations \
                 (re-exported as world_interaction::layer3::composition_rule::composer_registrations) \
                 or change the hook event_kind"
            ),
        ));
    }
}

fn collect_hook_intent_kinds(
    expr: &spellkave_dsl::Expression,
    node_id: spellkave_dsl::NodeId,
    source: HookExpressionSource,
    out: &mut BTreeSet<&'static str>,
    visited: &mut BTreeSet<spellkave_dsl::NodeId>,
) {
    if !visited.insert(node_id) {
        return;
    }
    let Some(node) = expr.node(node_id) else {
        return;
    };
    if let Some(tag) = side_effect_intent_tag(&node.op) {
        out.insert(tag);
    }
    if let (HookExpressionSource::Server, spellkave_dsl::Op::RollTable(roll)) = (source, &node.op) {
        collect_hook_intent_kinds(expr, roll.draw_node, source, out, visited);
        return;
    }
    for child in node.op.child_nodes() {
        collect_hook_intent_kinds(expr, child, source, out, visited);
    }
}

fn side_effect_intent_tag(op: &spellkave_dsl::Op) -> Option<&'static str> {
    use spellkave_dsl::Op;
    use spellkave_schema::{
        CONDITION_HOOK_INTENT_KIND_APPLY_STATUS, CONDITION_HOOK_INTENT_KIND_ARBITRATE,
        CONDITION_HOOK_INTENT_KIND_EMIT_TRACE, CONDITION_HOOK_INTENT_KIND_INVOKE_OUTCOME_GRAPH,
        CONDITION_HOOK_INTENT_KIND_MODIFY_RESOURCE, CONDITION_HOOK_INTENT_KIND_OUTCOME_SEQUENCE,
    };
    match op {
        Op::ModifyResource(_) => Some(CONDITION_HOOK_INTENT_KIND_MODIFY_RESOURCE),
        Op::ApplyStatus(_) => Some(CONDITION_HOOK_INTENT_KIND_APPLY_STATUS),
        Op::EmitTrace(_) => Some(CONDITION_HOOK_INTENT_KIND_EMIT_TRACE),
        Op::InvokeOutcomeGraph(_) => Some(CONDITION_HOOK_INTENT_KIND_INVOKE_OUTCOME_GRAPH),
        Op::OutcomeSequence(_) => Some(CONDITION_HOOK_INTENT_KIND_OUTCOME_SEQUENCE),
        Op::Arbitrate(_) => Some(CONDITION_HOOK_INTENT_KIND_ARBITRATE),
        _ => None,
    }
}

fn validate_hook_event_bindings(
    source: &str,
    event_kind: &str,
    path: &str,
    condition_definition_id: &str,
    macros: &[spellkave_dsl::DslMacroDefinition],
    diags: &mut Vec<Diagnostic>,
) {
    if source.is_empty() || parse_event_kind_path(event_kind).is_err() {
        return;
    }
    let Ok(expr) = dsl_macros::parse_predicate(source, macros) else {
        return;
    };
    let allowed = event_detail_fields(event_kind);
    if let Err(err) = validate_event_binding_refs_for_event(&expr, event_kind, &allowed) {
        match err {
            spellkave_dsl::EventBindingValidationError::UnknownDetailField {
                field_id,
                event_kind,
                ..
            } => diags.push(fatal(
                MF_VAL_EVENT_DETAIL_FIELD,
                path,
                format!(
                    "condition_definition '{}' hook for event_kind '{}' references unknown $event.detail.{} field",
                    condition_definition_id, event_kind, field_id
                ),
            )),
        }
    }
}

/// K-B Part A1: derived from the single declared table in
/// `spellkave_dsl::EVENT_DETAIL_FIELDS` — replaces the former hand-maintained
/// match that stopped at `Layer2.ApplyConsequence` and rejected every
/// `$event.detail.*` reference on `Layer2.ResourceDelta` (and any other event
/// kind) regardless of whether the producer actually publishes the field.
fn event_detail_fields(event_kind: &str) -> Vec<&'static str> {
    spellkave_dsl::event_detail_fields_for_kind(event_kind)
        .into_iter()
        .map(|field| field.field_id)
        .collect()
}

fn validate_event_kind_source(
    event_kind: &str,
    path: &str,
    condition_definition_id: &str,
    diags: &mut Vec<Diagnostic>,
) {
    if parse_event_kind_path(event_kind).is_ok() {
        return;
    }
    let message = if let Some((scope, variant)) = event_kind.split_once('.') {
        format!(
            "condition_definition '{}' hook references unknown {}Kind variant '{}' in event_kind '{}'; see spellkave_schema::ALL_EVENT_KIND_PATHS",
            condition_definition_id, scope, variant, event_kind
        )
    } else {
        format!(
            "condition_definition '{}' hook references unknown event_kind '{}'; see spellkave_schema::ALL_EVENT_KIND_PATHS",
            condition_definition_id, event_kind
        )
    };
    diags.push(fatal(MF_VAL_EVENT_KIND, path, message));
}

/// K-B Part A2: derived from `spellkave_schema::ALL_EVENT_KIND_PATHS` instead
/// of hand-duplicating the closed `EventKind` set a third time (server's
/// `world_event::parse_event_kind_path` is the authoritative parser; this
/// crate cannot depend on server, so it validates membership in the shared
/// table instead of re-encoding the enum shape). Fixes `Layer2.ResourceDelta`
/// (and any other post-#1332 kind) being rejected here even though the
/// runtime supports it.
fn parse_event_kind_path(value: &str) -> Result<(), ()> {
    if spellkave_schema::ALL_EVENT_KIND_PATHS.contains(&value) {
        Ok(())
    } else {
        Err(())
    }
}

fn validate_predicate_source(
    source: &str,
    path: &str,
    row_kind: &str,
    row_id: &str,
    macros: &[spellkave_dsl::DslMacroDefinition],
    diags: &mut Vec<Diagnostic>,
) {
    if source.is_empty() {
        return;
    }
    if let Err(err) = dsl_macros::parse_predicate(source, macros) {
        push_dsl_parse_diag(MF_VAL_PREDICATE_PARSE, path, row_kind, row_id, err, diags);
    }
}

fn validate_item_dsl_sources(manifest: &Manifest, diags: &mut Vec<Diagnostic>) {
    let macros = dsl_macros::dsl_macro_compiler_definitions(manifest);
    for row in &manifest.entity_definition_item_dsl_source {
        let row_id = format!("{}:{}", row.entity_def_id, row.slot);
        if row.source.trim().is_empty() {
            diags.push(fatal(
                MF_VAL_ITEM_DSL_SOURCE_PARSE,
                "entity_definition_item_dsl_source.source",
                format!("entity_definition_item_dsl_source '{row_id}': source must not be empty"),
            ));
            continue;
        }
        if let Err(err) = dsl_macros::parse_int_expression(&row.source, &macros) {
            push_dsl_parse_diag(
                MF_VAL_ITEM_DSL_SOURCE_PARSE,
                "entity_definition_item_dsl_source.source",
                "entity_definition_item_dsl_source",
                &row_id,
                err,
                diags,
            );
        }
    }
}

fn validate_outcome_source(
    source: &str,
    path: &str,
    row_kind: &str,
    row_id: &str,
    macros: &[spellkave_dsl::DslMacroDefinition],
    diags: &mut Vec<Diagnostic>,
) {
    if source.is_empty() {
        return;
    }
    if let Err(err) = dsl_macros::parse_outcome(source, macros) {
        push_dsl_parse_diag(MF_VAL_DSL_PARSE, path, row_kind, row_id, err, diags);
    }
}

fn push_dsl_parse_diag(
    code: &str,
    path: &str,
    row_kind: &str,
    row_id: &str,
    err: spellkave_dsl::ParseFailure,
    diags: &mut Vec<Diagnostic>,
) {
    let msg = err
        .diagnostics
        .first()
        .map(|diag| diag.message.clone())
        .unwrap_or_else(|| "parse error".to_owned());
    diags.push(Diagnostic {
        code: code.to_owned(),
        severity: Severity::Fatal,
        message: format!("{row_kind} '{row_id}': DSL parse error: {msg}"),
        path: path.to_owned(),
        span: None,
        hint: None,
        details: Some(serde_json::json!({
            "row_kind": row_kind,
            "row_id": row_id,
        })),
    });
}

fn validate_target_conditions(manifest: &Manifest, diags: &mut Vec<Diagnostic>) {
    let entity_ids = numeric_target_ids(manifest.entities.len(), &manifest.metadata, "entity_ids");
    // `target_kind = "item"` / `"world_object"` predate the unified entity
    // model and were only ever counted from the `[[items]]`/`[[world_objects]]`
    // row kinds retired in #1968 B-10 — those row counts are now permanently 0
    // (no manifest field populates them), so these two `target_kind` branches
    // are unreachable for any authored row. Kept for `target_kind` string-
    // vocabulary compatibility with the live kernel's `TargetKind::Item` /
    // `TargetKind::WorldObject` vocabulary,
    // which is unrelated to manifest authoring and out of this retirement's
    // scope. Follow-up: fold "item"/"world_object" target_conditions counting
    // onto `[[entities]]` (kind="item"/"world_object"), mirroring
    // `RefIndex.items` in `refs.rs`, if this ever needs to be author-reachable
    // again.
    let item_ids = numeric_target_ids(0, &manifest.metadata, "item_ids");
    let world_object_ids = numeric_target_ids(0, &manifest.metadata, "world_object_ids");

    for row in &manifest.target_conditions {
        let targets = match row.target_kind.as_str() {
            "entity" => &entity_ids,
            "item" => &item_ids,
            "world_object" => &world_object_ids,
            other => {
                diags.push(Diagnostic {
                    code: MF_VAL_TARGET_KIND.to_owned(),
                    severity: Severity::Fatal,
                    message: format!("unsupported target_condition target_kind '{other}'"),
                    path: "target_conditions.target_kind".to_owned(),
                    span: None,
                    hint: Some("use one of: entity, item, world_object".to_owned()),
                    details: Some(serde_json::json!({
                        "target_kind": other,
                    })),
                });
                continue;
            }
        };

        if !targets.contains(&row.target_id) {
            diags.push(Diagnostic {
                code: MF_VAL_FK_CLOSURE.to_owned(),
                severity: Severity::Fatal,
                message: format!(
                    "target_condition references missing {} target id {}",
                    row.target_kind, row.target_id
                ),
                path: "target_conditions.target_id".to_owned(),
                span: None,
                hint: None,
                details: Some(serde_json::json!({
                    "target_kind": row.target_kind,
                    "target_id": row.target_id,
                })),
            });
        }
    }
}

fn numeric_target_ids(
    row_count: usize,
    metadata: &BTreeMap<String, MetadataValue>,
    key: &str,
) -> BTreeSet<u64> {
    let mut ids = (1..=row_count as u64).collect::<BTreeSet<_>>();
    for value in parse_csv_list(metadata, key) {
        if let Ok(id) = value.parse::<u64>() {
            ids.insert(id);
        }
    }
    ids
}

pub fn diagnostics_to_canonical_json(
    diagnostics: &[Diagnostic],
) -> Result<String, serde_json::Error> {
    let mut sorted = diagnostics.to_vec();
    canonicalize_diagnostics(&mut sorted);
    serde_json::to_string(&sorted)
}

fn validate_legacy_metadata_value_types(
    metadata: &BTreeMap<String, MetadataValue>,
    diags: &mut Vec<Diagnostic>,
) {
    const LEGACY_STRING_KEYS: &[&str] = &[
        "entities",
        "fk_refs",
        "crossing_anchors",
        "cross_substrate_links",
        "poses",
        "nodes",
        "edges",
        "bundle_deps",
        "h3_ancestry",
        "location_parents",
        "entity_ids",
        "item_ids",
        "world_object_ids",
        "profile",
        "predicate",
        "outcome",
    ];
    for key in LEGACY_STRING_KEYS {
        if matches!(metadata.get(*key), Some(value) if value.as_str().is_none()) {
            diags.push(fatal(
                MF_VAL_METADATA_LEGACY_VALUE,
                &format!("metadata.{key}"),
                format!("legacy validation metadata key '{key}' must remain a string"),
            ));
        }
    }
}

fn validate_fk_closure(metadata: &BTreeMap<String, MetadataValue>, diags: &mut Vec<Diagnostic>) {
    let entities = parse_csv_set(metadata, "entities");
    for reference in parse_csv_list(metadata, "fk_refs") {
        let Some((left, right)) = reference.split_once("->") else {
            continue;
        };
        if !entities.contains(right) {
            diags.push(fatal(
                MF_VAL_FK_CLOSURE,
                "fk_refs",
                format!("unresolved reference {left}->{right}"),
            ));
        }
    }
}

fn validate_substrate_crossings(
    metadata: &BTreeMap<String, MetadataValue>,
    diags: &mut Vec<Diagnostic>,
) {
    let anchors = parse_csv_set(metadata, "crossing_anchors");
    for crossing in parse_csv_list(metadata, "cross_substrate_links") {
        if !anchors.contains(crossing.as_str()) {
            diags.push(fatal(
                MF_VAL_SUBSTRATE_CROSSING_ANCHOR,
                "cross_substrate_links",
                format!("cross-substrate link {crossing} requires anchor"),
            ));
        }
    }
}

fn validate_pose_rows(metadata: &BTreeMap<String, MetadataValue>, diags: &mut Vec<Diagnostic>) {
    for pose in parse_csv_list(metadata, "poses") {
        let parts: Vec<&str> = pose.split(':').collect();
        if parts.len() != 8 {
            diags.push(fatal(
                MF_VAL_POSE_CHECK,
                "poses",
                format!("invalid pose component count: {pose}"),
            ));
            continue;
        }
        let mut ok = true;
        for component in parts.iter().skip(1) {
            if component.parse::<f64>().is_err() {
                ok = false;
            }
        }
        if !ok {
            diags.push(fatal(
                MF_VAL_POSE_CHECK,
                "poses",
                format!("pose has non-numeric component: {pose}"),
            ));
        }
    }
}

fn validate_node_id_format(
    metadata: &BTreeMap<String, MetadataValue>,
    diags: &mut Vec<Diagnostic>,
) {
    for node in parse_csv_list(metadata, "nodes") {
        if !is_valid_node_id(node.as_str()) {
            diags.push(fatal(
                MF_VAL_NODE_ID_FORMAT,
                "nodes",
                format!("invalid node id: {node}"),
            ));
        }
    }
}

fn validate_edge_endpoints(
    metadata: &BTreeMap<String, MetadataValue>,
    diags: &mut Vec<Diagnostic>,
) {
    let nodes = parse_csv_set(metadata, "nodes");
    for edge in parse_csv_list(metadata, "edges") {
        let Some((_, endpoints)) = edge.split_once(':') else {
            continue;
        };
        let Some((from, to)) = endpoints.split_once("->") else {
            continue;
        };
        if !nodes.contains(from) || !nodes.contains(to) {
            diags.push(fatal(
                MF_VAL_EDGE_ENDPOINT,
                "edges",
                format!("edge endpoints must resolve: {edge}"),
            ));
        }
    }
}

fn validate_bundle_dag(metadata: &BTreeMap<String, MetadataValue>, diags: &mut Vec<Diagnostic>) {
    let edges = parse_pairs(metadata, "bundle_deps");
    let mut graph: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for (from, to) in &edges {
        graph.entry(from.as_str()).or_default().push(to.as_str());
        graph.entry(to.as_str()).or_default();
    }
    let mut temp = BTreeSet::new();
    let mut perm = BTreeSet::new();
    for node in graph.keys() {
        if visit_cycle(node, &graph, &mut temp, &mut perm) {
            diags.push(fatal(
                MF_VAL_BUNDLE_DAG,
                "bundle_deps",
                "bundle dependency cycle detected".to_owned(),
            ));
            return;
        }
    }
}

fn validate_h3_ancestry(metadata: &BTreeMap<String, MetadataValue>, diags: &mut Vec<Diagnostic>) {
    for link in parse_csv_list(metadata, "h3_ancestry") {
        let Some((child, parent)) = link.split_once("->") else {
            continue;
        };
        if child.len() <= parent.len() || !child.starts_with(parent) {
            diags.push(fatal(
                MF_VAL_H3_ANCESTRY,
                "h3_ancestry",
                format!("invalid h3 ancestry {child}->{parent}"),
            ));
        }
    }
}

fn validate_location_parent_dag(
    metadata: &BTreeMap<String, MetadataValue>,
    diags: &mut Vec<Diagnostic>,
) {
    let edges = parse_pairs(metadata, "location_parents");
    let mut graph: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for (child, parent) in &edges {
        graph
            .entry(child.as_str())
            .or_default()
            .push(parent.as_str());
        graph.entry(parent.as_str()).or_default();
    }
    let mut temp = BTreeSet::new();
    let mut perm = BTreeSet::new();
    for node in graph.keys() {
        if visit_cycle(node, &graph, &mut temp, &mut perm) {
            diags.push(fatal(
                MF_VAL_LOCATION_PARENT_DAG,
                "location_parents",
                "location parent cycle detected".to_owned(),
            ));
            return;
        }
    }
}

fn visit_cycle<'a>(
    node: &'a str,
    graph: &'a BTreeMap<&'a str, Vec<&'a str>>,
    temp: &mut BTreeSet<&'a str>,
    perm: &mut BTreeSet<&'a str>,
) -> bool {
    if perm.contains(node) {
        return false;
    }
    if !temp.insert(node) {
        return true;
    }
    if let Some(neighbors) = graph.get(node) {
        for neighbor in neighbors {
            if visit_cycle(neighbor, graph, temp, perm) {
                return true;
            }
        }
    }
    temp.remove(node);
    perm.insert(node);
    false
}

fn parse_csv_set(metadata: &BTreeMap<String, MetadataValue>, key: &str) -> BTreeSet<String> {
    parse_csv_list(metadata, key).into_iter().collect()
}

fn parse_csv_list(metadata: &BTreeMap<String, MetadataValue>, key: &str) -> Vec<String> {
    metadata
        .get(key)
        .and_then(MetadataValue::as_str)
        .map(|value| {
            value
                .split(',')
                .map(str::trim)
                .filter(|part| !part.is_empty())
                .map(ToOwned::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

fn parse_pairs(metadata: &BTreeMap<String, MetadataValue>, key: &str) -> Vec<(String, String)> {
    parse_csv_list(metadata, key)
        .into_iter()
        .filter_map(|edge| {
            edge.split_once("->")
                .map(|(a, b)| (a.trim().to_owned(), b.trim().to_owned()))
        })
        .collect()
}

fn is_valid_node_id(value: &str) -> bool {
    let Some((prefix, suffix)) = value.split_once(':') else {
        return false;
    };
    !prefix.is_empty()
        && !suffix.is_empty()
        && suffix
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
}

fn is_valid_place_node_id(node_id: &str, place_graph_id: &str, local_id: &str) -> bool {
    if place_graph_id.is_empty() || local_id.is_empty() || local_id.contains('/') {
        return false;
    }
    node_id == format!("{place_graph_id}/{local_id}")
}

fn fatal(code: &str, path: &str, message: String) -> Diagnostic {
    Diagnostic {
        code: code.to_owned(),
        severity: Severity::Fatal,
        message,
        path: path.to_owned(),
        span: None,
        hint: None,
        details: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use spellkave_schema::{
        AffordanceTemplateRow, ConditionDefinitionRow, ConditionRemovePolicyDefinitionRow,
        DslMacroDefinitionRow, EntityDefinitionItemDslSourceRow, EntityDefinitionRow,
        EntityKindRow, EntityRow, Manifest, ModifierTemplateRow, OutcomeTemplateRow,
        PromptDefinitionRow, ResourceDefinitionRow, SchemaVersion, SpatialNicheAttachmentRow,
        TargetConditionRow,
    };
    use std::collections::BTreeMap;

    fn affordance_template_row(
        template_id: &str,
        predicate_source: &str,
        accepts_global_storm: bool,
    ) -> AffordanceTemplateRow {
        AffordanceTemplateRow {
            template_id: template_id.to_string(),
            bundle_id: "core".to_string(),
            niche_id: String::new(),
            capability_id: String::new(),
            display_name: template_id.to_string(),
            predicate_bool: true,
            predicate_source: predicate_source.to_string(),
            accepts_global_storm,
            locality_class: String::new(),
            reads_occupancy: false,
            description: String::new(),
            stratification_ok: true,
            outcome_template_id: String::new(),
            range_m: 0.0,
            spends_channel: None,
            recharges_on: None,
            composes: None,

            movement_cost_expression_source: String::new(),
        }
    }

    fn prompt_definition_row(
        prompt_def_id: &str,
        open_predicate_source: &str,
        accepts_global_storm: bool,
    ) -> PromptDefinitionRow {
        PromptDefinitionRow {
            prompt_def_id: prompt_def_id.to_string(),
            bundle_id: "core".to_string(),
            source_selector: "core/npc".to_string(),
            open_predicate_source: open_predicate_source.to_string(),
            response_action_ids: vec!["core/answer".to_string()],
            audience_kind_ref: "core/audience/directed_trace".to_string(),
            trace_window_ticks: 10,
            contract_version: 1,
            accepts_global_storm,
        }
    }

    fn outcome_template_row(
        outcome_template_id: &str,
        manipulation_action_id: &str,
    ) -> OutcomeTemplateRow {
        OutcomeTemplateRow {
            outcome_template_id: outcome_template_id.to_owned(),
            bundle_id: "core".to_owned(),
            display_name: outcome_template_id.to_owned(),
            outcome_bool: true,
            outcome_expression_source: String::new(),
            manipulation_action_id: manipulation_action_id.to_owned(),
            communication_speech_act_id: String::new(),
            movement_descriptor: String::new(),
            sense_query_kind: String::new(),
            sense_visibility: spellkave_schema::VisibilityClass::Local,
            consequence_template_id: String::new(),
            discovery_validated: false,
            description: String::new(),
        }
    }

    fn spatial_niche_attachment_row() -> SpatialNicheAttachmentRow {
        SpatialNicheAttachmentRow {
            bundle_id: "core".to_owned(),
            place_node_id: "core/hall/plate".to_owned(),
            place_edge_id: None,
            niche_id: "core/pressure_plate".to_owned(),
            visibility: "Public".to_owned(),
            condition_bool: true,
            ecological_invariant: false,
            on_entry_action_id: "core/pressure_plate/save".to_owned(),
            on_entry_target_entity_key: "core/pressure_plate_1".to_owned(),
        }
    }

    #[test]
    fn spatial_entry_action_requires_both_authored_refs() {
        let mut manifest = Manifest::default();
        let mut row = spatial_niche_attachment_row();
        row.on_entry_target_entity_key.clear();
        manifest.spatial_niche_attachments.push(row);

        let diags = validate_manifest(&manifest);
        assert!(diags
            .iter()
            .any(|diag| diag.code == MF_VAL_SPATIAL_ENTRY_ACTION_SHAPE));
    }

    #[test]
    fn spatial_entry_action_rejects_edge_attachment() {
        let mut manifest = Manifest::default();
        let mut row = spatial_niche_attachment_row();
        row.place_node_id.clear();
        row.place_edge_id = Some(7);
        manifest.spatial_niche_attachments.push(row);

        let diags = validate_manifest(&manifest);
        assert!(diags
            .iter()
            .any(|diag| diag.code == MF_VAL_SPATIAL_ENTRY_ACTION_SHAPE));
    }

    #[test]
    fn spatial_entry_action_shape_accepts_paired_node_binding() {
        let mut manifest = Manifest::default();
        manifest
            .spatial_niche_attachments
            .push(spatial_niche_attachment_row());

        let diags = validate_manifest(&manifest);
        assert!(!diags
            .iter()
            .any(|diag| diag.code == MF_VAL_SPATIAL_ENTRY_ACTION_SHAPE));
    }

    #[test]
    fn default_affordance_range_is_admitted() {
        let mut manifest = Manifest::default();
        manifest
            .affordance_templates
            .push(affordance_template_row("core/touch", "", false));
        let diags = validate_manifest(&manifest);
        assert!(
            !diags.iter().any(|d| d.code == MF_VAL_AFFORDANCE_RANGE),
            "default range_m=0.0 must be admitted; diags: {diags:?}"
        );
    }

    #[test]
    fn negative_affordance_range_is_rejected() {
        let mut manifest = Manifest::default();
        let mut row = affordance_template_row("core/bad_range", "", false);
        row.range_m = -1.0;
        manifest.affordance_templates.push(row);
        let diags = validate_manifest(&manifest);
        assert!(
            diags.iter().any(|d| d.code == MF_VAL_AFFORDANCE_RANGE),
            "negative range_m must be rejected; diags: {diags:?}"
        );
    }

    /// D1/D8 (actor-carried-verbs): locality classification is purely a
    /// function of what the predicate itself reads (ADR-0018 D-1: no
    /// kernel-minted enum, no branch on `niche_id`'s namespace here). An
    /// actor-provided (`capability:` namespace) niche whose predicate reads
    /// no `target.*` fact classifies `Local` and validates with no
    /// diagnostics — no attachment is demanded for it (Gate B, not the
    /// validator, was ever the blocker; V1 confirmed this live).
    #[test]
    fn capability_namespace_niche_with_actor_only_predicate_classifies_local() {
        let mut manifest = Manifest::default();
        let mut row = affordance_template_row(
            "srd/attack/melee/affordance",
            r#"actor.has_capability("srd/action/attack")"#,
            false,
        );
        row.niche_id = "capability:srd/action/attack".to_string();
        manifest.affordance_templates.push(row);

        let classes = classify_affordance_templates(&manifest);
        assert_eq!(
            classes,
            vec![(
                "srd/attack/melee/affordance".to_string(),
                LocalityClass::Local
            )]
        );

        let mut diags = Vec::new();
        validate_affordance_predicate_locality(&manifest, &mut diags);
        assert!(
            diags.is_empty(),
            "an actor-only predicate on a capability: niche must not be flagged: {diags:?}"
        );
    }

    /// D8 regression, the reverse case: a thing-provided niche (any
    /// non-`capability:` namespace) whose predicate reads `target.*` keeps
    /// classifying `Place` exactly as today — the provider axis changes Gate
    /// B admission (server-side), never locality classification.
    #[test]
    fn thing_provided_niche_with_target_read_still_classifies_place() {
        let mut manifest = Manifest::default();
        let mut row = affordance_template_row(
            "core/open/affordance",
            r#"target.has_niche("core/chest")"#,
            false,
        );
        row.niche_id = "core/openable".to_string();
        manifest.affordance_templates.push(row);

        let classes = classify_affordance_templates(&manifest);
        assert_eq!(
            classes,
            vec![("core/open/affordance".to_string(), LocalityClass::Place)]
        );
    }

    #[test]
    fn ranged_manipulation_template_satisfies_reachability_parity_contract() {
        let mut manifest = Manifest::default();
        manifest.outcome_templates.push(outcome_template_row(
            "core/ranged/outcome",
            "core/ranged_action",
        ));
        let mut row = affordance_template_row("core/ranged", "", false);
        row.outcome_template_id = "core/ranged/outcome".to_owned();
        row.range_m = 9.0;
        manifest.affordance_templates.push(row);
        let diags = validate_manifest(&manifest);
        assert!(
            !diags.iter().any(|d| d.code == MF_VAL_REACHABILITY_PARITY),
            "manipulation-scoped range_m should satisfy parity check; diags: {diags:?}"
        );
    }

    #[test]
    fn ranged_non_manipulation_template_fails_reachability_parity_contract() {
        let mut manifest = Manifest::default();
        manifest
            .outcome_templates
            .push(outcome_template_row("core/sense/outcome", ""));
        let mut row = affordance_template_row("core/ranged_sense", "", false);
        row.outcome_template_id = "core/sense/outcome".to_owned();
        row.range_m = 9.0;
        manifest.affordance_templates.push(row);
        let diags = validate_manifest(&manifest);
        assert!(
            diags.iter().any(|d| d.code == MF_VAL_REACHABILITY_PARITY),
            "non-manipulation range_m must fail parity check; diags: {diags:?}"
        );
    }

    // ── locality classifier tests (task 1.6) ────────────────────────────────

    #[test]
    fn place_bound_predicate_is_place_class() {
        // target.has_niche reads from co-located entity → Place
        let mut manifest = Manifest::default();
        manifest.affordance_templates.push(affordance_template_row(
            "core/perceive_chest",
            r#"target.has_niche("core/chest")"#,
            false,
        ));
        let diags = validate_manifest(&manifest);
        let codes: Vec<&str> = diags.iter().map(|d| d.code.as_str()).collect();
        assert!(
            !codes.contains(&MF_VAL_GLOBAL_STORM_UNANNOTATED),
            "place-bound predicate must not require global storm annotation; diags: {diags:?}"
        );
    }

    #[test]
    fn lambda_projection_is_global() {
        // world.state reads global singleton → Global; no annotation → reject
        let mut manifest = Manifest::default();
        manifest.affordance_templates.push(affordance_template_row(
            "bad/global_predicate",
            r#"world.state("storming") == true"#,
            false,
        ));
        let diags = validate_manifest(&manifest);
        let codes: Vec<&str> = diags.iter().map(|d| d.code.as_str()).collect();
        assert!(
            codes.contains(&MF_VAL_GLOBAL_STORM_UNANNOTATED),
            "global predicate without annotation must be rejected; diags: {diags:?}"
        );
        let diag = diags
            .iter()
            .find(|d| d.code == MF_VAL_GLOBAL_STORM_UNANNOTATED)
            .unwrap();
        assert!(
            diag.message.contains("bad/global_predicate"),
            "diagnostic must name the template"
        );
        let details = diag.details.as_ref().unwrap();
        assert_eq!(details["template_id"], "bad/global_predicate");
        assert_eq!(details["locality_class"], "Global");
        assert_eq!(details["missing_annotation"], "accepts_global_storm");
    }

    #[test]
    fn unannotated_global_is_rejected() {
        let mut manifest = Manifest::default();
        manifest.affordance_templates.push(affordance_template_row(
            "core/global_test",
            r#"world.state("storming") == true"#,
            false, // no annotation
        ));
        let diags = validate_manifest(&manifest);
        assert!(
            diags
                .iter()
                .any(|d| d.code == MF_VAL_GLOBAL_STORM_UNANNOTATED),
            "unannotated Global must be rejected"
        );
    }

    #[test]
    fn annotated_global_is_admitted() {
        let mut manifest = Manifest::default();
        manifest.affordance_templates.push(affordance_template_row(
            "core/global_test",
            r#"world.state("storming") == true"#,
            true, // annotated
        ));
        let diags = validate_manifest(&manifest);
        assert!(
            !diags
                .iter()
                .any(|d| d.code == MF_VAL_GLOBAL_STORM_UNANNOTATED),
            "annotated Global must be admitted"
        );
    }

    // ── prompt_definition locality/stratification tests (ADR-0142 D-1/D-8 gate 8) ──

    #[test]
    fn prompt_definition_local_selector_needs_no_annotation() {
        // actor.stat reads the speaker's own resource → Local.
        let mut manifest = Manifest::default();
        manifest.prompt_definitions.push(prompt_definition_row(
            "dm/creation/species",
            r#"actor.stat("dm/dialog/open") >= 1"#,
            false,
        ));
        let diags = validate_manifest(&manifest);
        assert!(
            !diags
                .iter()
                .any(|d| d.code == MF_VAL_GLOBAL_STORM_UNANNOTATED),
            "a speaker-own-state selector must not require global storm annotation; diags: {diags:?}"
        );
    }

    #[test]
    fn prompt_definition_unannotated_global_selector_is_rejected() {
        let mut manifest = Manifest::default();
        manifest.prompt_definitions.push(prompt_definition_row(
            "bad/global_prompt",
            r#"world.state("storming") == true"#,
            false, // no annotation
        ));
        let diags = validate_manifest(&manifest);
        assert!(
            diags
                .iter()
                .any(|d| d.code == MF_VAL_GLOBAL_STORM_UNANNOTATED),
            "unannotated Global open_predicate_source must be rejected; diags: {diags:?}"
        );
    }

    #[test]
    fn prompt_definition_annotated_global_selector_is_admitted() {
        let mut manifest = Manifest::default();
        manifest.prompt_definitions.push(prompt_definition_row(
            "core/global_prompt",
            r#"world.state("storming") == true"#,
            true, // annotated
        ));
        let diags = validate_manifest(&manifest);
        assert!(
            !diags
                .iter()
                .any(|d| d.code == MF_VAL_GLOBAL_STORM_UNANNOTATED),
            "annotated Global open_predicate_source must be admitted"
        );
    }

    #[test]
    fn prompt_definition_parse_error_is_reported() {
        let mut manifest = Manifest::default();
        manifest.prompt_definitions.push(prompt_definition_row(
            "bad/parse_prompt",
            "actor.stat(", // malformed
            true,
        ));
        let diags = validate_manifest(&manifest);
        assert!(
            diags.iter().any(|d| d.code == MF_VAL_PREDICATE_PARSE),
            "malformed open_predicate_source must be reported; diags: {diags:?}"
        );
    }

    #[test]
    fn predicate_reading_affordance_table_is_rejected_with_cycle_path() {
        // The DSL has no direct "read affordance table" op today; the
        // stratification check catches this via the graph analysis.
        // We simulate it by injecting edges directly.
        use stratification::{
            check_stratification, MaterialiserWriteEdge, PredicateReadEdge, AFFORDANCE_TABLE,
        };
        let read_edges = vec![PredicateReadEdge {
            predicate_id: "p_bad".into(),
            table_name: AFFORDANCE_TABLE.into(),
        }];
        let write_edges = vec![MaterialiserWriteEdge {
            rule_id: "mat/p_bad".into(),
            target_table: AFFORDANCE_TABLE.into(),
        }];
        let result = check_stratification(&read_edges, &write_edges, AFFORDANCE_TABLE);
        assert!(!result.ok, "cycle must be detected");
        assert!(
            !result.cycle_nodes.is_empty(),
            "cycle path must be non-empty"
        );
        assert!(
            result.cycle_nodes.iter().any(|n| n == AFFORDANCE_TABLE),
            "cycle path must name the affordance table"
        );
    }

    fn manifest_with_metadata(entries: &[(&str, &str)]) -> Manifest {
        let metadata = entries
            .iter()
            .map(|(key, value)| {
                (
                    (*key).to_owned(),
                    MetadataValue::String((*value).to_owned()),
                )
            })
            .collect::<BTreeMap<_, _>>();
        Manifest {
            schema_version: SchemaVersion::new(1, 0, 0),
            metadata,
            ..Manifest::default()
        }
    }

    fn modifier_template_row(modifier_template_id: &str) -> ModifierTemplateRow {
        ModifierTemplateRow {
            modifier_template_id: modifier_template_id.to_owned(),
            bundle_id: "core".to_owned(),
            applies_to: "ResolutionCheck".to_owned(),
            display_name: modifier_template_id.to_owned(),
            predicate_source: r#"actor.has_capability("core/endure")"#.to_owned(),
            modifier_expression_source: r#"actor.has_capability("core/endure")"#.to_owned(),
            priority: 0,
            delta_kind: "Plus".to_owned(),
            replace_template_id: None,
            description: String::new(),
        }
    }

    fn condition_definition_row(
        condition_definition_id: &str,
        modifier_template_id: &str,
    ) -> ConditionDefinitionRow {
        ConditionDefinitionRow {
            condition_definition_id: condition_definition_id.to_owned(),
            bundle_id: "core".to_owned(),
            display_name: condition_definition_id.to_owned(),
            stacking_rule: "core/stacking/single".to_owned(),
            remove_time_policy: "core/remove_policy/independent_lifetime".to_owned(),
            gate_predicate_source: r#"actor.has_capability("core/endure")"#.to_owned(),
            modifier_template_id: modifier_template_id.to_owned(),
            hooks: Vec::new(),
            observable: true,
            grants_capability_while_active: Vec::new(),
            suppresses_capability_while_active: Vec::new(),
            implies: Vec::new(),
            description: String::new(),
        }
    }

    fn target_condition_row(target_kind: &str, target_id: u64) -> TargetConditionRow {
        TargetConditionRow {
            target_kind: target_kind.to_owned(),
            target_id,
            condition_definition_id: "core/poisoned".to_owned(),
            source: "test".to_owned(),
            stack_count: 1,
            expires_at_tick: None,
        }
    }

    fn valid_poisoned_condition_manifest() -> Manifest {
        let mut manifest = Manifest::default();
        manifest
            .condition_remove_policy_definitions
            .push(ConditionRemovePolicyDefinitionRow {
                policy_id: "core/remove_policy/independent_lifetime".to_owned(),
                bundle_id: "core".to_owned(),
                display_name: "Independent lifetime".to_owned(),
                remove_predicate_source: "0 == 1".to_owned(),
                description: String::new(),
            });
        manifest
            .modifier_templates
            .push(modifier_template_row("core/poisoned_penalty"));
        manifest
            .condition_definitions
            .push(condition_definition_row(
                "core/poisoned",
                "core/poisoned_penalty",
            ));
        manifest.entities.push(EntityRow {
            // Namespaced under the row's own `bundle_id` ("core") since
            // MF-VAL-291 (#1510) flags a definitional id defined outside its
            // owning bundle's namespace.
            entity_key: "core/npc/rat".to_owned(),
            bundle_id: "core".to_owned(),
            entity_def_id: String::new(),
            name: "Rat".to_owned(),
            kind: EntityKindRow::Npc,
            description: String::new(),
            place_node_id: String::new(),
            quantity: 1,
            owned_by_entity_key: String::new(),
        });
        manifest
    }

    #[test]
    fn implied_by_count_literal_condition_ref_is_rejected_when_unknown() {
        let mut manifest = valid_poisoned_condition_manifest();
        manifest.condition_remove_policy_definitions[0].remove_predicate_source =
            r#"implied_by_count("core/condition/missing") == 0"#.to_owned();

        let diagnostics = validate_manifest(&manifest);

        assert!(
            diagnostics.iter().any(|diag| {
                diag.code == refs::MF_VAL_CONDITION_IMPLIES_REF
                    && diag.path == "condition_remove_policy_definitions.remove_predicate_source"
            }),
            "unknown implied_by_count child id must be rejected: {diagnostics:?}"
        );
    }

    #[test]
    fn implied_by_count_literal_condition_ref_passes_when_declared() {
        let mut manifest = valid_poisoned_condition_manifest();
        manifest.condition_remove_policy_definitions[0].remove_predicate_source =
            r#"implied_by_count("core/poisoned") == 0"#.to_owned();

        let diagnostics = validate_manifest(&manifest);

        assert!(
            diagnostics.is_empty(),
            "declared implied_by_count child id must pass: {diagnostics:?}"
        );
    }

    #[test]
    fn delta_kind_garbage_is_fatal() {
        // CR-3 / ADR-0038 (COND-2): an authored delta_kind outside the kernel's
        // ModifierDeltaTag set is rejected at admit-time, NOT silently fail-soft'd
        // to a dropped modifier at runtime.
        let mut manifest = valid_poisoned_condition_manifest();
        manifest.modifier_templates[0].delta_kind = "Garbage".to_owned();
        let diags = validate_manifest(&manifest);
        assert!(
            diags
                .iter()
                .any(|d| d.code == MF_VAL_MODIFIER_TEMPLATE_DELTA_KIND
                    && d.severity == Severity::Fatal),
            "delta_kind='Garbage' must emit MF-VAL-243 Fatal, got {diags:?}"
        );
    }

    #[test]
    fn delta_kind_replace_is_accepted() {
        // The closed set admits every ModifierDeltaTag arm — Replace included.
        let mut manifest = valid_poisoned_condition_manifest();
        manifest.modifier_templates[0].delta_kind = "Replace".to_owned();
        let diags = validate_manifest(&manifest);
        assert!(
            !diags
                .iter()
                .any(|d| d.code == MF_VAL_MODIFIER_TEMPLATE_DELTA_KIND),
            "delta_kind='Replace' is a legal ModifierDeltaTag, got {diags:?}"
        );
    }

    fn entity_definition_row(entity_def_id: &str, stackable: bool) -> EntityDefinitionRow {
        EntityDefinitionRow {
            entity_def_id: entity_def_id.to_owned(),
            bundle_id: "core".to_owned(),
            display_name: entity_def_id.to_owned(),
            lineage_id: String::new(),
            affordance_seed_id: String::new(),
            on_consume_consequence_id: String::new(),
            on_destroy_consequence_id: String::new(),
            on_equip_consequence_id: String::new(),
            on_unequip_consequence_id: String::new(),
            grants_capability_while_equipped: String::new(),
            grants_capability_while_present_in_location: String::new(),
            slot_compatibility_predicate: String::new(),
            stackable,
            description: String::new(),
        }
    }

    #[test]
    fn quantity_gt_one_requires_stackable_entity_definition() {
        let mut manifest = Manifest::default();
        manifest
            .entity_definitions
            .push(entity_definition_row("core/coin", false));
        manifest.entities.push(EntityRow {
            entity_key: "coin_stack".to_owned(),
            bundle_id: "core".to_owned(),
            entity_def_id: "core/coin".to_owned(),
            name: "Coin stack".to_owned(),
            kind: EntityKindRow::Item,
            description: String::new(),
            place_node_id: String::new(),
            quantity: 23,
            owned_by_entity_key: String::new(),
        });

        let diags = validate_manifest(&manifest);
        let diag = diags
            .iter()
            .find(|d| d.code == MF_VAL_STACKABLE_QUANTITY)
            .expect("non-stackable quantity > 1 should be rejected");
        assert!(
            diag.message
                .contains("'stackable = true' on entity_definition 'core/coin'"),
            "got: {diag:?}"
        );

        manifest.entity_definitions[0].stackable = true;
        let diags = validate_manifest(&manifest);
        assert!(
            !diags.iter().any(|d| d.code == MF_VAL_STACKABLE_QUANTITY),
            "stackable definition should admit quantity > 1: {diags:?}"
        );
    }

    #[test]
    fn canonicalization_order_is_stable() {
        let mut diags = vec![
            Diagnostic {
                code: "MF-VAL-999".to_owned(),
                severity: Severity::Fatal,
                message: "z".to_owned(),
                path: "z.path".to_owned(),
                span: Some(Span {
                    start_line: 4,
                    start_col: 8,
                    end_line: 4,
                    end_col: 9,
                }),
                hint: None,
                details: None,
            },
            Diagnostic {
                code: "MF-VAL-001".to_owned(),
                severity: Severity::Fatal,
                message: "a".to_owned(),
                path: "a.path".to_owned(),
                span: Some(Span {
                    start_line: 1,
                    start_col: 1,
                    end_line: 1,
                    end_col: 5,
                }),
                hint: None,
                details: None,
            },
        ];
        canonicalize_diagnostics(&mut diags);
        assert_eq!(diags[0].code, "MF-VAL-001");
        assert_eq!(diags[1].code, "MF-VAL-999");
    }

    #[test]
    fn mf_code_is_stable_for_unsupported_version() {
        let diags = validate_manifest_version("2.0.0");
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, MF_VAL_UNSUPPORTED_SCHEMA_VERSION);
    }

    #[test]
    fn validator_enforces_manifest_invariants() {
        let manifest = manifest_with_metadata(&[
            ("entities", "node:a"),
            ("fk_refs", "edge:e1->node:missing"),
            ("cross_substrate_links", "a->b"),
            ("crossing_anchors", "x->y"),
            ("poses", "node:a:1:2"),
            ("nodes", "node:a,bad"),
            ("edges", "edge1:node:a->node:missing"),
            ("bundle_deps", "core->addon,addon->core"),
            ("h3_ancestry", "abcd->wxyz"),
            ("location_parents", "town->inn,inn->town"),
        ]);
        let diagnostics = validate_manifest(&manifest);
        let codes: Vec<&str> = diagnostics.iter().map(|diag| diag.code.as_str()).collect();
        assert!(codes.contains(&MF_VAL_FK_CLOSURE));
        assert!(codes.contains(&MF_VAL_SUBSTRATE_CROSSING_ANCHOR));
        assert!(codes.contains(&MF_VAL_POSE_CHECK));
        assert!(codes.contains(&MF_VAL_NODE_ID_FORMAT));
        assert!(codes.contains(&MF_VAL_EDGE_ENDPOINT));
        assert!(codes.contains(&MF_VAL_BUNDLE_DAG));
        assert!(codes.contains(&MF_VAL_H3_ANCESTRY));
        assert!(codes.contains(&MF_VAL_LOCATION_PARENT_DAG));
    }

    #[test]
    fn typed_rows_validate_edge_endpoint_and_crossing_anchor() {
        let mut manifest = manifest_with_metadata(&[]);
        manifest.bundles.push(spellkave_schema::BundleRow {
            bundle_id: "bad".into(),
            display_name: "Bad".into(),
            author: "test".into(),
            version: "0.0.0".into(),
            depends_on: Vec::new(),
            description: String::new(),
            tag_prefixes: Vec::new(),
            allow_flat_tags: false,
            default_extent_shape_id: String::new(),
            default_extent_params_json: String::new(),
        });
        manifest.place_graphs.push(spellkave_schema::PlaceGraphRow {
            place_graph_id: "bad_graph".into(),
            bundle_id: "bad".into(),
            substrate_id: "outdoor".into(),
            display_name: "Bad Graph".into(),
            origin_cell_res10: 0,
            origin_cell_res7: 0,
            origin_cell_res4: 0,
            description: String::new(),
            bearing: 0.0,
            metres_per_unit: 1.0,
            blueprint_id: String::new(),
            parent_graph_id: String::new(),
            anchor_node_id: String::new(),
            is_ephemeral_instance: false,
        });
        manifest.place_nodes.push(spellkave_schema::PlaceNodeRow {
            node_id: "bad_graph/start".into(),
            bundle_id: "bad".into(),
            place_graph_id: "bad_graph".into(),
            local_id: "start".into(),
            substrate_id: "outdoor".into(),
            display_name: "Start".into(),
            description: String::new(),
            cell_res10: 0,
            cell_res7: 0,
            cell_res4: 0,
            pose_elevation: 0.0,
            pose_elevation_present: false,
            pos_x: 0,
            pos_y: 0,
            pos_present: false,
            extent_shape_id: String::new(),
            extent_params_json: String::new(),
            is_ephemeral: false,
        });
        manifest.place_edges.push(spellkave_schema::PlaceEdgeRow {
            edge_key: "bad/edge/missing".into(),
            bundle_id: "bad".into(),
            place_graph_id: "bad_graph".into(),
            from_node_id: "bad_graph/start".into(),
            to_node_id: "bad_graph/missing".into(),
            pose_dt_elevation: 0.0,
            pose_dt_elevation_present: false,
            substrate_crossing: true,
            cell_anchor: 0,
            cell_anchor_u: 0.0,
            cell_anchor_v: 0.0,
            cell_anchor_present: true,
            from_anchor_side: String::new(),
            from_anchor_offset: 0.0,
            to_anchor_side: String::new(),
            to_anchor_offset: 0.0,
            description: String::new(),

            inherit_movement_default: true,
        });

        let diagnostics = validate_manifest(&manifest);
        let codes: Vec<&str> = diagnostics.iter().map(|diag| diag.code.as_str()).collect();
        assert!(codes.contains(&MF_VAL_EDGE_ENDPOINT));
        assert!(codes.contains(&MF_VAL_SUBSTRATE_CROSSING_ANCHOR));
    }

    #[test]
    fn canonical_json_output_is_stable() {
        let diagnostics = vec![
            Diagnostic {
                code: "MF-VAL-200".to_owned(),
                severity: Severity::Fatal,
                message: "z".to_owned(),
                path: "z".to_owned(),
                span: None,
                hint: None,
                details: None,
            },
            Diagnostic {
                code: "MF-VAL-100".to_owned(),
                severity: Severity::Fatal,
                message: "a".to_owned(),
                path: "a".to_owned(),
                span: None,
                hint: None,
                details: None,
            },
        ];
        let json = diagnostics_to_canonical_json(&diagnostics).expect("should serialize");
        assert!(
            json.find("MF-VAL-100").unwrap_or(usize::MAX) < json.find("MF-VAL-200").unwrap_or(0)
        );
    }

    #[test]
    fn valid_poisoned_condition_manifest_is_admitted() {
        let manifest = valid_poisoned_condition_manifest();
        let diagnostics = validate_manifest(&manifest);
        assert!(
            diagnostics.is_empty(),
            "valid poisoned condition authoring must pass: {diagnostics:?}"
        );
    }

    #[test]
    fn target_conditions_accept_all_admitted_target_kinds() {
        let mut manifest = valid_poisoned_condition_manifest();
        manifest
            .target_conditions
            .push(target_condition_row("entity", 1));
        let diagnostics = validate_manifest(&manifest);
        assert!(
            diagnostics.is_empty(),
            "entity target should be admitted: {diagnostics:?}"
        );
    }

    #[test]
    fn target_conditions_item_and_world_object_kinds_are_now_permanently_unresolvable() {
        // #1968 B-10 retired the `[[items]]`/`[[world_objects]]` manifest row
        // kinds that used to back these two `target_kind` string values'
        // numeric target-id counting (a pre-unification positional scheme,
        // never tied to `manifest_authored_entity`/entity_id). The vocabulary
        // strings themselves stay accepted (kernel-level `TargetKind::Item` /
        // `TargetKind::WorldObject` compatibility, out of this retirement's
        // scope) but can never resolve any target_id now that the row kinds
        // that populated their counts are gone.
        for target_kind in ["item", "world_object"] {
            let mut manifest = valid_poisoned_condition_manifest();
            manifest
                .target_conditions
                .push(target_condition_row(target_kind, 1));
            let diagnostics = validate_manifest(&manifest);
            assert!(
                diagnostics
                    .iter()
                    .any(|diag| diag.code == MF_VAL_FK_CLOSURE),
                "{target_kind} target_kind should now be permanently unresolvable: {diagnostics:?}"
            );
        }
    }

    #[test]
    fn target_condition_unknown_target_kind_is_rejected() {
        let mut manifest = valid_poisoned_condition_manifest();
        manifest
            .target_conditions
            .push(target_condition_row("actor", 1));
        let diagnostics = validate_manifest(&manifest);
        assert!(
            diagnostics
                .iter()
                .any(|diag| diag.code == MF_VAL_TARGET_KIND),
            "unknown target kind must be rejected: {diagnostics:?}"
        );
    }

    #[test]
    fn target_condition_missing_target_id_is_rejected() {
        let mut manifest = valid_poisoned_condition_manifest();
        manifest
            .target_conditions
            .push(target_condition_row("world_object", 99));
        let diagnostics = validate_manifest(&manifest);
        assert!(
            diagnostics.iter().any(|diag| {
                diag.code == MF_VAL_FK_CLOSURE && diag.path == "target_conditions.target_id"
            }),
            "missing target id must be rejected: {diagnostics:?}"
        );
    }

    #[test]
    fn target_condition_unresolved_condition_definition_is_rejected() {
        let mut manifest = valid_poisoned_condition_manifest();
        manifest.condition_definitions.clear();
        manifest
            .target_conditions
            .push(target_condition_row("entity", 1));
        let diagnostics = validate_manifest(&manifest);
        assert!(
            diagnostics.iter().any(|diag| {
                diag.code == MF_VAL_FK_CLOSURE
                    && diag.path == "target_conditions.condition_definition_id"
            }),
            "unresolved condition_definition_id must be rejected: {diagnostics:?}"
        );
    }

    #[test]
    fn condition_definition_unresolved_modifier_template_is_rejected() {
        let mut manifest = valid_poisoned_condition_manifest();
        manifest.modifier_templates.clear();
        let diagnostics = validate_manifest(&manifest);
        assert!(
            diagnostics.iter().any(|diag| {
                diag.code == MF_VAL_FK_CLOSURE
                    && diag.path == "condition_definitions.modifier_template_id"
            }),
            "unresolved modifier_template_id must be rejected: {diagnostics:?}"
        );
    }

    #[test]
    fn condition_definition_hooks_validate_event_kind_taxonomy() {
        let mut manifest = valid_poisoned_condition_manifest();
        manifest.condition_definitions[0]
            .hooks
            .push(spellkave_schema::ConditionHookRow {
                event_kind: "Layer2.ApplyConsequence".to_owned(),
                detail_filter: Some(1),
                detail_filter_source: None,
                expression: Some(2),
                expression_source: None,
            });
        assert!(
            validate_manifest(&manifest)
                .iter()
                .all(|diag| diag.path != "condition_definitions.hooks.event_kind"),
            "valid event_kind must be accepted"
        );

        manifest.condition_definitions[0].hooks[0].event_kind = "Layer2.Combat".to_owned();
        let diagnostics = validate_manifest(&manifest);
        assert!(
            diagnostics.iter().any(|diag| {
                diag.code == MF_VAL_EVENT_KIND
                    && diag.path == "condition_definitions.hooks.event_kind"
                    && diag.message.contains("unknown Layer2Kind variant 'Combat'")
                    && diag
                        .message
                        .contains("spellkave_schema::ALL_EVENT_KIND_PATHS")
            }),
            "unknown event_kind must be rejected with taxonomy diagnostic: {diagnostics:?}"
        );
    }

    #[test]
    fn condition_definition_hooks_reject_unknown_event_detail_fields() {
        let mut manifest = valid_poisoned_condition_manifest();
        manifest.condition_definitions[0].gate_predicate_source =
            "$event.detail.unknown == 'x'".to_owned();
        manifest.condition_definitions[0]
            .hooks
            .push(spellkave_schema::ConditionHookRow {
                event_kind: "Layer2.ApplyConsequence".to_owned(),
                detail_filter: Some(1),
                detail_filter_source: None,
                expression: Some(2),
                expression_source: None,
            });

        let diagnostics = validate_manifest(&manifest);
        assert!(
            diagnostics.iter().any(|diag| {
                diag.code == MF_VAL_EVENT_DETAIL_FIELD
                    && diag.path == "condition_definitions.hooks.detail_filter"
                    && diag.message.contains("$event.detail.unknown")
            }),
            "unknown event detail field must be rejected: {diagnostics:?}"
        );
    }

    #[test]
    fn legacy_status_definition_id_field_is_rejected_by_strict_schema() {
        let condition = serde_json::json!({
            "condition_definition_id": "core/poisoned",
            "bundle_id": "core",
            "display_name": "Poisoned",
            "stacking_rule": "core/stacking/single",
            "gate_predicate_source": "true",
            "status_definition_id": "legacy"
        });
        let err = serde_json::from_value::<ConditionDefinitionRow>(condition)
            .expect_err("legacy status_definition_id must be rejected");
        assert!(err.to_string().contains("unknown field"));
    }

    /// close-behavior-loop task 0.9 — ADR-0024 multi-runtime contract.
    /// A hook whose modifier intent has no registered §5.19 composer for the
    /// chosen `event_kind` must be rejected at admission, not silently dropped
    /// at runtime.
    #[test]
    fn condition_hook_modifier_intent_for_unregistered_event_kind_is_rejected() {
        let mut manifest = valid_poisoned_condition_manifest();
        manifest.condition_definitions[0]
            .hooks
            .push(spellkave_schema::ConditionHookRow {
                event_kind: "Tick".to_owned(),
                detail_filter: None,
                detail_filter_source: None,
                expression: Some(1),
                expression_source: None,
            });
        let diagnostics = validate_manifest(&manifest);
        assert!(
            diagnostics
                .iter()
                .any(|diag| diag.code == MF_VAL_HOOK_REACHABILITY
                    && diag.path == "condition_definitions.hooks.expression"
                    && diag.message.contains("Tick")
                    && diag.message.contains("ModifierDelta")),
            "unregistered (event_kind, intent_kind) pair must be rejected: {diagnostics:?}"
        );
    }

    #[test]
    fn shared_hook_reachability_preserves_both_walks() {
        use spellkave_dsl::{
            EmitTrace, Expression, ExpressionKind, Node, Op, RollTable, SubjectRef, ValueType,
        };
        use spellkave_schema::{
            CONDITION_HOOK_INTENT_KIND_EMIT_TRACE, CONDITION_HOOK_INTENT_KIND_MODIFIER_DELTA,
        };

        // This hand-built AST puts an intent behind the table-id edge to
        // distinguish the two walks. All admitted event kinds have an EmitTrace
        // composer, so use an unknown event to expose the walk's result.
        let expr = Expression {
            kind: ExpressionKind::Predicate,
            root_node_id: 1,
            nodes: vec![
                Node {
                    node_id: 1,
                    value_type: ValueType::Int,
                    op: Op::RollTable(RollTable {
                        table_id_node: 2,
                        draw_node: 3,
                    }),
                    span: Default::default(),
                },
                Node {
                    node_id: 2,
                    value_type: ValueType::Outcome,
                    op: Op::EmitTrace(EmitTrace {
                        subject: SubjectRef::Actor,
                        trace_kind_id: "core/test".into(),
                    }),
                    span: Default::default(),
                },
            ],
        };
        assert_eq!(
            unregistered_hook_intent_kinds(
                "UnknownEvent",
                "",
                1,
                &expr,
                HookExpressionSource::Manifest
            ),
            BTreeSet::from([CONDITION_HOOK_INTENT_KIND_EMIT_TRACE]),
        );
        assert!(unregistered_hook_intent_kinds(
            "UnknownEvent",
            "",
            1,
            &expr,
            HookExpressionSource::Server
        )
        .is_empty());
        assert_eq!(
            unregistered_hook_intent_kinds(
                "Tick",
                "core/modifier",
                3,
                &expr,
                HookExpressionSource::Server
            ),
            BTreeSet::from([CONDITION_HOOK_INTENT_KIND_MODIFIER_DELTA]),
        );
    }

    #[test]
    fn invalid_condition_dsl_fields_are_rejected() {
        let mut manifest = valid_poisoned_condition_manifest();
        manifest.condition_definitions[0].gate_predicate_source = "target.".to_owned();
        manifest.modifier_templates[0].predicate_source = "world.".to_owned();
        manifest.modifier_templates[0].modifier_expression_source = "outcome.".to_owned();
        let diagnostics = validate_manifest(&manifest);
        assert!(
            diagnostics
                .iter()
                .any(|diag| diag.path == "condition_definitions.gate_predicate_source"),
            "invalid gate predicate must be rejected: {diagnostics:?}"
        );
        assert!(
            diagnostics
                .iter()
                .any(|diag| diag.path == "modifier_templates.predicate_source"),
            "invalid modifier predicate must be rejected: {diagnostics:?}"
        );
        assert!(
            diagnostics
                .iter()
                .any(|diag| diag.path == "modifier_templates.modifier_expression_source"),
            "invalid modifier expression must be rejected: {diagnostics:?}"
        );
    }

    /// ADR-0142 D-1/D-4: `audience_kind_ref` must resolve (against the two
    /// core seeds or an authored `audience_kind_definitions` row), and every
    /// `response_action_ids` entry must name an existing `action_definitions`
    /// row. `response_action_ids` resolution itself now lives in
    /// `refs::validate_prompt_response_action_refs` (sk-jay0.2.33.15.7,
    /// canon-warden A-4), so this exercises the full closed-world pass
    /// (`validate_manifest`), not `validate_typed_manifest_rows` alone.
    #[test]
    fn prompt_definition_audience_and_response_action_refs_are_checked() {
        let action = action_with_cost(0);
        let known_action_id = action.action_id.clone();
        let mut manifest = Manifest {
            // `action_with_cost` authors `required_capability: "core/ritual"`;
            // the full `validate_manifest` pass this test now runs (unlike
            // `action_with_cost`'s other callers, which drive a single narrow
            // check function) also resolves that ref, so it needs a matching
            // `capability_definitions` row or MF-VAL-265 fires and pollutes the
            // "known response action id must not be flagged" assertion below —
            // unrelated to what this test proves.
            capability_definitions: vec![spellkave_schema::CapabilityDefinitionRow {
                capability_id: "core/ritual".into(),
                bundle_id: "core".into(),
                actor_kind: "Player".into(),
                display_name: "Ritual".into(),
                description: String::new(),
                default_visibility: "public".into(),
            }],
            action_definitions: vec![action],
            prompt_definitions: vec![spellkave_schema::PromptDefinitionRow {
                prompt_def_id: "dm/prompt/creation_phase_1".into(),
                bundle_id: "core".into(),
                source_selector: "npc/aldwin".into(),
                open_predicate_source: String::new(),
                response_action_ids: vec![known_action_id.clone(), "core/unknown_response".into()],
                audience_kind_ref: "core/audience/unknown".into(),
                trace_window_ticks: 20,
                contract_version: 1,
                accepts_global_storm: false,
            }],
            ..Manifest::default()
        };

        let diagnostics = validate_manifest(&manifest);

        assert!(
            diagnostics
                .iter()
                .any(|d| d.code == MF_VAL_FK_CLOSURE
                    && d.path == "prompt_definitions.audience_kind_ref"),
            "unresolved audience_kind_ref must be rejected: {diagnostics:?}"
        );
        assert!(
            diagnostics.iter().any(|d| d.code == MF_VAL_FK_CLOSURE
                && d.path == "prompt_definitions.response_action_ids"
                && d.message.contains("core/unknown_response")),
            "unresolved response_action_ids entry must be rejected: {diagnostics:?}"
        );
        assert!(
            !diagnostics
                .iter()
                .any(|d| d.message.contains(known_action_id.as_str())),
            "the known response action id must not be flagged: {diagnostics:?}"
        );

        // The two core audience kinds resolve without an authored row.
        manifest.prompt_definitions[0].audience_kind_ref = "core/audience/broadcast".into();
        manifest.prompt_definitions[0].response_action_ids = vec![known_action_id];
        let diagnostics = validate_manifest(&manifest);
        assert!(
            diagnostics.is_empty(),
            "core/audience/broadcast and a known response action must both resolve: {diagnostics:?}"
        );
    }

    /// sk-jay0.2.33.15.7: a delta's `prompt_definitions.response_action_ids`
    /// may name an already-published DEPENDENCY's action (not re-declared in
    /// this delta) — it must resolve against `live.actions` (existence-only,
    /// same posture as `live.economy_channels`/`live.modifier_templates`),
    /// same as the closed-world case must still reject an id absent from
    /// both the manifest and the live world.
    #[test]
    fn response_action_ids_resolve_against_the_live_world_index() {
        let manifest = Manifest {
            // "elven_mini" must be a DECLARED bundle (an undeclared `bundle_id`
            // trips its OWN MF-VAL-101 "references unknown bundle" FK check
            // before response_action_ids resolution is even reached) with
            // `depends_on: ["srd"]` — the real elven-mini/srd relationship —
            // so the live action below resolves through BOTH existence and
            // `BundleScope::in_scope`, not existence alone. Using "core" here
            // instead would UNDER-test: "core" is force-registered with a
            // closure of just itself (nothing legitimately depends ON it that
            // way round), so a "srd" action would wrongly look out-of-scope.
            // Scope's REJECTION path is
            // `response_action_ids_out_of_scope_action_is_cross_bundle_scope_rejected`
            // below.
            bundles: vec![
                spellkave_schema::BundleRow {
                    bundle_id: "elven_mini".into(),
                    display_name: "Elven Mini".into(),
                    author: "spellkave-test".into(),
                    version: "1.0.0".into(),
                    depends_on: vec!["srd".into()],
                    description: String::new(),
                    tag_prefixes: vec![],
                    allow_flat_tags: false,
                    default_extent_shape_id: String::new(),
                    default_extent_params_json: String::new(),
                },
                // "srd" itself is only DECLARED here (its `[[bundles]]` row, so
                // "elven_mini"'s `depends_on` resolves) — its action_definitions
                // are NOT part of this delta, only of the live world below, the
                // same "already-published dependency" shape the test's own
                // doc comment describes.
                spellkave_schema::BundleRow {
                    bundle_id: "srd".into(),
                    display_name: "SRD".into(),
                    author: "spellkave-test".into(),
                    version: "1.0.0".into(),
                    depends_on: vec![],
                    description: String::new(),
                    tag_prefixes: vec![],
                    allow_flat_tags: false,
                    default_extent_shape_id: String::new(),
                    default_extent_params_json: String::new(),
                },
            ],
            prompt_definitions: vec![spellkave_schema::PromptDefinitionRow {
                prompt_def_id: "elven_mini/prompt/quest_1".into(),
                bundle_id: "elven_mini".into(),
                source_selector: "npc/aldwin".into(),
                open_predicate_source: String::new(),
                response_action_ids: vec!["srd/dm/dialog/choice/1".into()],
                audience_kind_ref: "core/audience/broadcast".into(),
                trace_window_ticks: 20,
                contract_version: 1,
                accepts_global_storm: false,
            }],
            ..Manifest::default()
        };

        // Closed world unchanged: absent from both the delta's own
        // `action_definitions` and the live world, still MF-VAL-101.
        let diagnostics = validate_manifest_with_world(&manifest, &BTreeSet::new(), None);
        assert!(
            diagnostics.iter().any(|d| d.code == MF_VAL_FK_CLOSURE
                && d.path == "prompt_definitions.response_action_ids"
                && d.message.contains("srd/dm/dialog/choice/1")),
            "an id absent from both the manifest and an absent live world must still be rejected: {diagnostics:?}"
        );

        // Present only in the world's live index (the dependency bundle that
        // declared it is already published, not part of this delta): passes.
        let mut live = refs::LiveVocabulary::default();
        live.actions
            .insert("srd/dm/dialog/choice/1".into(), "srd".into());
        let diagnostics = validate_manifest_with_world(&manifest, &BTreeSet::new(), Some(&live));
        assert!(
            diagnostics.is_empty(),
            "a response_action_ids entry present only in the live world index must resolve: {diagnostics:?}"
        );
    }

    /// sk-jay0.2.33.15.7 (canon-warden A-4): `response_action_ids` resolves
    /// through the SAME existence-plus-bundle-scope path as the sibling live
    /// maps — a prompt may only name actions in its OWN bundle's `depends_on`
    /// closure. An action that exists live in a bundle OUTSIDE that closure
    /// is rejected `MF_VAL_CROSS_BUNDLE_SCOPE` (MF-VAL-214), not silently
    /// admitted just because it exists somewhere in the world.
    #[test]
    fn response_action_ids_out_of_scope_action_is_cross_bundle_scope_rejected() {
        let manifest = Manifest {
            bundles: vec![spellkave_schema::BundleRow {
                bundle_id: "test".into(),
                display_name: "Test".into(),
                author: "spellkave-test".into(),
                version: "1.0.0".into(),
                depends_on: vec![],
                description: String::new(),
                tag_prefixes: vec![],
                allow_flat_tags: false,
                default_extent_shape_id: String::new(),
                default_extent_params_json: String::new(),
            }],
            prompt_definitions: vec![spellkave_schema::PromptDefinitionRow {
                prompt_def_id: "test/prompt/1".into(),
                bundle_id: "test".into(),
                source_selector: "npc/aldwin".into(),
                open_predicate_source: String::new(),
                response_action_ids: vec!["other/dialog/choice/1".into()],
                audience_kind_ref: "core/audience/broadcast".into(),
                trace_window_ticks: 20,
                contract_version: 1,
                accepts_global_storm: false,
            }],
            ..Manifest::default()
        };
        let mut live = refs::LiveVocabulary::default();
        live.actions
            .insert("other/dialog/choice/1".into(), "other_bundle".into());
        let diagnostics = validate_manifest_with_world(&manifest, &BTreeSet::new(), Some(&live));
        assert!(
            diagnostics
                .iter()
                .any(|d| d.code == refs::MF_VAL_CROSS_BUNDLE_SCOPE
                    && d.path == "prompt_definitions.response_action_ids"
                    && d.message.contains("other/dialog/choice/1")),
            "an action outside 'test' bundle's depends_on closure must be rejected \
             cross-bundle-scope, even though it exists live: {diagnostics:?}"
        );
    }

    #[test]
    fn audience_kind_definition_resolution_shape_and_bundle_are_checked() {
        let mut manifest = Manifest {
            audience_kind_definitions: vec![spellkave_schema::AudienceKindDefinitionRow {
                audience_kind_id: "srd/audience/party_wide".into(),
                resolution_shape: "NotAShape".into(),
                bundle_id: "unknown_bundle".into(),
                description: String::new(),
            }],
            ..Manifest::default()
        };
        let mut diagnostics = Vec::new();
        validate_typed_manifest_rows(&manifest, &BTreeSet::new(), None, &mut diagnostics);
        assert!(diagnostics.iter().any(|d| d.code == MF_VAL_FK_CLOSURE
            && d.path == "audience_kind_definitions.resolution_shape"));
        assert!(diagnostics.iter().any(
            |d| d.code == MF_VAL_FK_CLOSURE && d.path == "audience_kind_definitions.bundle_id"
        ));

        manifest.audience_kind_definitions[0].resolution_shape = "Broadcast".into();
        manifest.audience_kind_definitions[0].bundle_id = "core".into();
        diagnostics.clear();
        validate_typed_manifest_rows(&manifest, &BTreeSet::new(), None, &mut diagnostics);
        assert!(
            diagnostics.is_empty(),
            "a well-formed audience_kind_definition row must pass: {diagnostics:?}"
        );
    }

    fn minimal_place_node(
        node_id: &str,
        bundle_id: &str,
        place_graph_id: &str,
        local_id: &str,
    ) -> spellkave_schema::PlaceNodeRow {
        serde_json::from_value(serde_json::json!({
            "node_id": node_id,
            "bundle_id": bundle_id,
            "place_graph_id": place_graph_id,
            "local_id": local_id,
            "substrate_id": "indoor",
            "display_name": "Test Node",
        }))
        .unwrap()
    }

    /// sk-jay0.7.174.23: a delta redeclaring an existing `place_node` sends
    /// only the changed row, never its `[[bundles]]`/`[[place_graphs]]`
    /// headers (`sk apply -k`'s configure-only send). `LiveVocabulary.bundles`/
    /// `.place_graphs` must resolve it instead of false-fataling MF-VAL-101.
    #[test]
    fn place_node_resolves_via_live_bundle_and_graph_without_local_headers() {
        let manifest = Manifest {
            place_nodes: vec![minimal_place_node(
                "elven_mini/inn/tavern",
                "elven_mini",
                "elven_mini/inn",
                "tavern",
            )],
            ..Manifest::default()
        };
        let live = refs::LiveVocabulary {
            bundles: BTreeSet::from(["elven_mini".to_string()]),
            place_graphs: BTreeMap::from([(
                "elven_mini/inn".to_string(),
                "elven_mini".to_string(),
            )]),
            ..Default::default()
        };
        let mut diagnostics = Vec::new();
        validate_typed_manifest_rows(&manifest, &BTreeSet::new(), Some(&live), &mut diagnostics);
        assert!(
            diagnostics.is_empty(),
            "a place_node whose bundle/graph are live (not locally declared) must resolve: {diagnostics:?}"
        );
    }

    /// The live index is existence-only: a bundle/graph absent from BOTH the
    /// delta and the live set must stay fatal (no vacuous check).
    #[test]
    fn place_node_with_unknown_bundle_and_graph_stays_fatal_even_with_a_live_index_present() {
        let manifest = Manifest {
            place_nodes: vec![minimal_place_node(
                "elven_mini/inn/tavern",
                "elven_mini",
                "elven_mini/inn",
                "tavern",
            )],
            ..Manifest::default()
        };
        let live = refs::LiveVocabulary {
            bundles: BTreeSet::from(["some_other_bundle".to_string()]),
            place_graphs: BTreeMap::from([(
                "some/other/graph".to_string(),
                "some_other_bundle".to_string(),
            )]),
            ..Default::default()
        };
        let mut diagnostics = Vec::new();
        validate_typed_manifest_rows(&manifest, &BTreeSet::new(), Some(&live), &mut diagnostics);
        assert!(diagnostics
            .iter()
            .any(|d| d.code == MF_VAL_FK_CLOSURE && d.path == "place_nodes.bundle_id"));
        assert!(diagnostics
            .iter()
            .any(|d| d.code == MF_VAL_FK_CLOSURE && d.path == "place_nodes.place_graph_id"));
    }

    fn action_with_cost(cost_time_ms: u64) -> spellkave_schema::ActionDefinitionRow {
        serde_json::from_value(serde_json::json!({
            "action_id": "core/ritual",
            "bundle_id": "core",
            "required_capability": "core/ritual",
            "on_success": "core/ritual/success",
            "display_name": "Ritual",
            "cost_time_ms": cost_time_ms
        }))
        .expect("minimal action row")
    }

    #[test]
    fn non_steering_override_requires_a_real_non_self_target_action() {
        let mut action = action_with_cost(0);
        action.non_steering = true;
        action.on_success.clear();
        let mut manifest = Manifest {
            action_definitions: vec![action.clone()],
            ..Manifest::default()
        };
        let mut diagnostics = Vec::new();
        validate_action_non_steering_shape(&manifest, &mut diagnostics);
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, MF_VAL_ACTION_NON_STEERING_SHAPE);

        action.on_success = "core/help/success".into();
        action.self_only = true;
        manifest.action_definitions = vec![action];
        diagnostics.clear();
        validate_action_non_steering_shape(&manifest, &mut diagnostics);
        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("self_only"));
    }

    #[test]
    fn long_action_warning_respects_the_strict_threshold() {
        let mut manifest = Manifest {
            action_definitions: vec![action_with_cost(5_000)],
            ..Manifest::default()
        };
        let mut diagnostics = Vec::new();
        validate_long_action_promotion(&manifest, &mut diagnostics);
        assert!(diagnostics.is_empty());

        manifest.action_definitions = vec![action_with_cost(5_001)];
        validate_long_action_promotion(&manifest, &mut diagnostics);
        assert_eq!(diagnostics.len(), 1);
        let warning = &diagnostics[0];
        assert_eq!(warning.code, MF_VAL_LONG_ACTION_PROMOTION);
        assert_eq!(warning.severity, Severity::Warning);
        assert_eq!(warning.path, "action_definitions.cost_time_ms");
        assert_eq!(
            warning.details.as_ref().unwrap()["promotion_target"],
            "core/long_running_action"
        );
    }

    #[test]
    fn long_action_warning_names_the_authored_session_target() {
        let mut action = action_with_cost(600_000);
        action.session_def_id = "core/concentration".into();
        let manifest = Manifest {
            action_definitions: vec![action],
            ..Manifest::default()
        };
        let mut diagnostics = Vec::new();
        validate_long_action_promotion(&manifest, &mut diagnostics);
        assert!(diagnostics[0].message.contains("core/concentration"));
    }

    fn stochastic_action(
        predicate: &str,
        on_success: &str,
        on_failure: &str,
    ) -> spellkave_schema::ActionDefinitionRow {
        serde_json::from_value(serde_json::json!({
            "action_id": "core/lockpick",
            "bundle_id": "core",
            "required_capability": "core/lockpicking",
            "on_success": on_success,
            "on_failure": on_failure,
            "resolution_mode": "Stochastic",
            "resolution_predicate_source": predicate,
            "display_name": "Lockpick"
        }))
        .expect("stochastic action row")
    }

    #[test]
    fn stochastic_empty_resolution_predicate_is_rejected() {
        let manifest = Manifest {
            action_definitions: vec![stochastic_action("", "core/ok", "core/fail")],
            ..Manifest::default()
        };
        let diagnostics = validate_manifest(&manifest);
        assert!(
            diagnostics.iter().any(|d| {
                d.code == MF_VAL_STOCHASTIC_RESOLUTION_SHAPE
                    && d.message.contains("resolution_predicate_source")
            }),
            "{diagnostics:?}"
        );
    }

    #[test]
    fn stochastic_non_bool_resolution_predicate_is_rejected() {
        let manifest = Manifest {
            // Int expression must fail Bool parse.
            action_definitions: vec![stochastic_action("d20", "core/ok", "core/fail")],
            ..Manifest::default()
        };
        let diagnostics = validate_manifest(&manifest);
        assert!(
            diagnostics.iter().any(|d| {
                d.path.contains("resolution_predicate_source")
                    && (d.code == MF_VAL_PREDICATE_PARSE || d.code == MF_VAL_DSL_PARSE)
            }),
            "{diagnostics:?}"
        );
    }

    #[test]
    fn stochastic_missing_on_failure_is_rejected() {
        let manifest = Manifest {
            action_definitions: vec![stochastic_action("d20 >= 11", "core/ok", "")],
            ..Manifest::default()
        };
        let diagnostics = validate_manifest(&manifest);
        assert!(
            diagnostics.iter().any(|d| {
                d.code == MF_VAL_STOCHASTIC_RESOLUTION_SHAPE && d.message.contains("on_failure")
            }),
            "{diagnostics:?}"
        );
    }

    #[test]
    fn stochastic_missing_on_success_is_rejected() {
        let manifest = Manifest {
            action_definitions: vec![stochastic_action("d20 >= 11", "", "core/fail")],
            ..Manifest::default()
        };
        let diagnostics = validate_manifest(&manifest);
        assert!(
            diagnostics.iter().any(|d| {
                d.code == MF_VAL_STOCHASTIC_RESOLUTION_SHAPE && d.message.contains("on_success")
            }),
            "{diagnostics:?}"
        );
    }

    #[test]
    fn double_roll_shape_raises_warning_diagnostic() {
        let manifest = Manifest {
            action_definitions: vec![stochastic_action("d20 >= 11", "core/ok", "core/fail")],
            outcome_templates: vec![serde_json::from_value(serde_json::json!({
                "outcome_template_id": "core/lockpick/outcome",
                "bundle_id": "core",
                "display_name": "Lockpick",
                "manipulation_action_id": "core/lockpick"
            }))
            .expect("outcome")],
            affordance_templates: vec![serde_json::from_value(serde_json::json!({
                "template_id": "core/lockpick/affordance",
                "bundle_id": "core",
                "niche_id": "core/lock",
                "capability_id": "core/lockpicking",
                "outcome_template_id": "core/lockpick/outcome",
                "display_name": "Lockpick",
                "predicate_source": "d20 >= 5",
                "range_m": 0.0
            }))
            .expect("affordance")],
            ..Manifest::default()
        };
        let diagnostics = validate_manifest(&manifest);
        assert!(
            diagnostics.iter().any(|d| {
                d.code == MF_VAL_STOCHASTIC_DOUBLE_ROLL && d.severity == Severity::Warning
            }),
            "{diagnostics:?}"
        );
    }

    #[test]
    fn unknown_macro_in_item_dsl_source_is_fatal() {
        let mut manifest = Manifest::default();
        manifest
            .entity_definition_item_dsl_source
            .push(EntityDefinitionItemDslSourceRow {
                entity_def_id: "srd/weapon/battleaxe".into(),
                slot: "srd/damage".into(),
                source: "1d8 + macro('srd/strength_modifier')".into(),
                bundle_id: "srd".into(),
            });
        let diags = validate_manifest(&manifest);
        assert!(
            diags.iter().any(|d| d.code == MF_VAL_ITEM_DSL_SOURCE_PARSE),
            "unknown item-DSL-source macro must emit MF-VAL-318; got {diags:?}"
        );
    }

    #[test]
    fn known_macro_in_item_dsl_source_is_admitted() {
        let mut manifest = Manifest::default();
        manifest.dsl_macro_definitions.push(DslMacroDefinitionRow {
            macro_id: "srd/strength_modifier".into(),
            bundle_id: "srd".into(),
            return_type: spellkave_schema::DslValueType::Int,
            parameters: Vec::new(),
            body_source: "((actor.stat('srd/str') - 10) / 2)".into(),
            description: String::new(),
        });
        manifest.resource_definitions.push(ResourceDefinitionRow {
            resource_definition_id: "srd/str".into(),
            bundle_id: "srd".into(),
            display_name: "STR".into(),
            value_kind: "int".into(),
            min_value: 1,
            max_value: 30,
            recovery_rule: "none".into(),
            initial_value: None,
            description: String::new(),
        });
        manifest
            .entity_definition_item_dsl_source
            .push(EntityDefinitionItemDslSourceRow {
                entity_def_id: "srd/weapon/battleaxe".into(),
                slot: "srd/damage".into(),
                source: "1d8 + macro('srd/strength_modifier')".into(),
                bundle_id: "srd".into(),
            });
        let diags = validate_manifest(&manifest);
        assert!(
            !diags.iter().any(|d| {
                d.code == MF_VAL_ITEM_DSL_SOURCE_PARSE || d.code == MF_VAL_DSL_FORMULA_STAT_REF
            }),
            "declared item-DSL-source macro with a catalogued stat must admit; got {diags:?}"
        );
    }
}

pub mod dm_artifact;
pub mod dm_consequence_references;
pub mod dm_presentation;
pub mod dm_source;
pub mod dm_srd;
