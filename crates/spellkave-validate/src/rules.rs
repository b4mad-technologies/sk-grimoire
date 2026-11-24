//! Ordered manifest rule registry for offline and admission validation.

use super::{
    action_economy, affordance_selection, affordance_template_participants, area_shapes,
    autonomy_jobs, containment_policy, dm_acts, dsl_macros, equipped_numeric, instance_owner,
    metadata_attachments, observation, refs, retractions, tag_grammar, trace_kinds,
    validate_action_non_steering_shape, validate_affordance_predicate_locality,
    validate_affordance_predicate_stratification, validate_affordance_range_m,
    validate_condition_authoring, validate_item_dsl_sources, validate_long_action_promotion,
    validate_manifest_metadata, validate_prompt_predicate_locality,
    validate_prompt_predicate_stratification, validate_reachability_parity,
    validate_spatial_entry_action_shape, validate_stochastic_action_resolution,
    validate_typed_manifest_rows, visibility, Diagnostic,
};
use spellkave_schema::Manifest;
use std::collections::BTreeSet;

pub(super) struct ValidationContext<'a> {
    pub(super) manifest: &'a Manifest,
    pub(super) applying_manifest: &'a Manifest,
    pub(super) existing_node_ids: &'a BTreeSet<String>,
    pub(super) live: Option<&'a refs::LiveVocabulary>,
    pub(super) containment_context: Option<&'a refs::LiveVocabulary>,
}

enum Rule {
    Manifest(fn(&Manifest, &mut Vec<Diagnostic>)),
    Context(fn(&ValidationContext<'_>, &mut Vec<Diagnostic>)),
}

// Preserve declaration order across validation modes, including the order of
// diagnostics with equal canonical sort keys.
const RULES: &[Rule] = &[
    Rule::Context(|ctx, diags| {
        validate_typed_manifest_rows(ctx.manifest, ctx.existing_node_ids, ctx.live, diags)
    }),
    Rule::Context(|ctx, diags| instance_owner::validate(ctx.applying_manifest, diags)),
    Rule::Context(|ctx, diags| {
        containment_policy::validate(
            ctx.manifest,
            ctx.applying_manifest,
            ctx.containment_context,
            diags,
        )
    }),
    Rule::Context(|ctx, diags| refs::place_seals::validate(ctx.applying_manifest, ctx.live, diags)),
    Rule::Manifest(area_shapes::validate_area_shape_params),
    Rule::Context(|ctx, diags| {
        dm_acts::validate_for_apply(ctx.manifest, ctx.applying_manifest, ctx.live, diags)
    }),
    Rule::Manifest(dsl_macros::validate_dsl_macro_definitions),
    Rule::Manifest(validate_affordance_range_m),
    Rule::Manifest(validate_reachability_parity),
    Rule::Manifest(validate_long_action_promotion),
    Rule::Manifest(validate_stochastic_action_resolution),
    Rule::Manifest(validate_action_non_steering_shape),
    Rule::Manifest(validate_spatial_entry_action_shape),
    Rule::Manifest(|manifest, diags| {
        diags.extend(validate_manifest_metadata(&manifest.metadata));
    }),
    Rule::Manifest(validate_condition_authoring),
    Rule::Manifest(validate_item_dsl_sources),
    Rule::Manifest(equipped_numeric::validate),
    Rule::Manifest(validate_affordance_predicate_locality),
    Rule::Manifest(validate_affordance_predicate_stratification),
    Rule::Manifest(validate_prompt_predicate_locality),
    Rule::Manifest(validate_prompt_predicate_stratification),
    Rule::Context(|ctx, diags| {
        refs::validate_cross_row_refs_for_apply(
            ctx.manifest,
            ctx.applying_manifest,
            ctx.live,
            diags,
        )
    }),
    Rule::Context(|ctx, diags| {
        affordance_template_participants::validate_affordance_template_participants(
            ctx.manifest,
            ctx.live,
            diags,
        )
    }),
    Rule::Manifest(action_economy::validate_action_economy),
    Rule::Manifest(affordance_selection::validate_affordance_selection_ambiguity),
    Rule::Manifest(affordance_selection::validate_stochastic_affordance_overlap),
    Rule::Manifest(affordance_selection::validate_duplicate_niche_outcome_templates),
    Rule::Manifest(autonomy_jobs::validate_autonomy_jobs),
    Rule::Manifest(trace_kinds::validate_trace_kind_definitions),
    Rule::Manifest(trace_kinds::validate_place_node_trace_kind_closure),
    Rule::Manifest(retractions::validate_retractions),
    Rule::Manifest(observation::validate_observation_placeholders),
    Rule::Manifest(tag_grammar::validate_tag_vocabulary),
    Rule::Manifest(metadata_attachments::validate_metadata_attachment_target_kinds),
    Rule::Manifest(metadata_attachments::validate_metadata_namespace_audiences),
    Rule::Manifest(visibility::validate_visibility_tokens),
];

pub(super) fn run(context: &ValidationContext<'_>, diags: &mut Vec<Diagnostic>) {
    for rule in RULES {
        match rule {
            Rule::Manifest(validate) => validate(context.manifest, diags),
            Rule::Context(validate) => validate(context, diags),
        }
    }
}
