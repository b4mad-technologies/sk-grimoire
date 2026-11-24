//! Action-economy authoring validation (substrate batch 2, slice A; reset
//! boundaries reshaped by CR-12 / LR-1).
//!
//! Validates the two manifest authoring sections that slice A lands end-to-end
//! into the live #433 layer-3 tables: `economy_channel_definitions` and
//! `turn_scheduler`.
//!
//! `economy_channel_definitions.resets_on` resolves against **authored**
//! `boundary_definitions` rows, not a Rust `const`. This is the batch-2
//! warden condition-4 carrier, corrected: slice A shipped `resets_on`'s legal
//! set as `const RESETS_ON_LEGAL: &[&str]` (`short_rest`/`long_rest` — 5e rest
//! vocabulary baked into Rust), which satisfied the condition's letter ("never
//! a Rust enum") but missed its cited `anchor_kind_definition` pattern (legal
//! set as authored rows). CR-12 moves the vocabulary into `core/` and `srd/`
//! TOML so a modder adds a reset boundary (e.g. `encounter_end`) by authoring a
//! row, with no engine change (ADR-0018 D-1).
//!
//! Resolution reuses the SAME `depends_on` closure (`reference_scope::BundleScope`) every
//! cross-bundle reference uses — not a second cross-bundle rule. Two tiers:
//!   1. **scope** — a `resets_on` whose boundary IS declared in this manifest
//!      but in a bundle outside the row's `depends_on` closure is rejected.
//!   2. **existence** — a `resets_on` declared by NO boundary row in this
//!      manifest is rejected ONLY when the full vocabulary is visible (every
//!      bundle in the row's closure is present, e.g. the `check-refs` concat
//!      gate or a self-contained bundle like `core` validated alone). When a
//!      dependency's declarations are absent (a downstream bundle validated
//!      per-file), the token is trusted — the same leniency cross-row refs use
//!      for absent targets, so `just validate <downstream-file>` never
//!      false-rejects a boundary its dependency owns.
//!
//! `scheduler_kind` keeps its `const` legal set: `round_robin` is a substrate
//! concept, not world vocabulary, and is not the 5e-shape CR-12 targets.
//! Slice B reuses this module's boundary resolution for `recharges_on` /
//! `fire_at_boundary` (warden condition 7) — do not re-introduce a const there.
//!
//! The authored action-economy fields on `affordance_template` /
//! `action_definition` (`spends_channel`, `recharges_on`, `composes`,
//! `reaction_eligibility`, `reaction_trigger`) deserialize in slice A but their
//! validation co-locates with their deferred server-table storage in the
//! consuming slice (FK → C, recharge → B/C, compose → D, reaction-eligibility →
//! E, reaction-trigger → F-impl). Codes MF-VAL-234..239 are reserved for these.

use crate::reference_scope::{resolve_scoped_ref, BundleScope, RefResolution};
use crate::{Diagnostic, Severity};
use spellkave_schema::{EconomyChannelDefinitionRow, Manifest};
use std::collections::{BTreeMap, BTreeSet};

/// `economy_channel_definitions.resets_on` does not resolve to an in-scope
/// `boundary_definitions` row.
pub const MF_VAL_CHANNEL_RESETS_ON: &str = "MF-VAL-230";
/// Two `economy_channel_definitions` rows share an `id`.
pub const MF_VAL_CHANNEL_DUP: &str = "MF-VAL-231";
/// `turn_scheduler.scheduler_kind` is not a member of the legal set.
pub const MF_VAL_TURN_SCHEDULER_KIND: &str = "MF-VAL-232";
/// More than one `turn_scheduler` row in this manifest has `active = true`.
pub const MF_VAL_TURN_SCHEDULER_ACTIVE: &str = "MF-VAL-233";
/// `affordance_template.spends_channel` does not resolve to an in-scope
/// `economy_channel_definitions` row (slice C — the budget-gate FK).
pub const MF_VAL_SPENDS_CHANNEL: &str = "MF-VAL-234";
/// `affordance_template.recharges_on` does not resolve to an in-scope
/// `boundary_definitions` row (slice C — the recharge boundary, reusing
/// the same authored-vocabulary resolution as `resets_on`; warden condition 7).
pub const MF_VAL_RECHARGES_ON: &str = "MF-VAL-235";
/// `affordance_template.composes` names a sub-id that does not resolve to an
/// in-scope `affordance_template` (slice D — the composition FK; reuses the same
/// two-tier scoped resolution as the other action-economy fields).
pub const MF_VAL_COMPOSES_REF: &str = "MF-VAL-236";
/// `affordance_template.composes` must name leaf actions. Compositions
/// are single-level. An unresolved child in an absent dependency is deferred.
pub const MF_VAL_COMPOSES_NESTED: &str = "MF-VAL-237";
/// `action_definition.reaction_eligibility` classifies as `Global` locality
/// (slice E — warden condition 6). The reaction producer scans candidate
/// reactors against this predicate on every triggering event; a Global-class
/// predicate is an unbounded fan-out (ADR-0027 D-3). Unlike `affordance_template`
/// there is no `accepts_global_storm` escape hatch on `action_definition` — a
/// reaction predicate MUST be bounded (Local/Place/Family), so Global is Fatal.
pub const MF_VAL_REACTION_ELIGIBILITY_GLOBAL: &str = "MF-VAL-238";
/// `action_definition.reaction_trigger` reads unbounded world state (slice
/// F-impl — the §8.4 WHAT-provokes axis). The producer evaluates this predicate
/// ONCE per triggering event against the fixed intent facts (`$event.*`), so
/// `$event.*` reads are bounded (`classify_reaction_trigger` treats them Local);
/// but a `world.*`/`session.*` read would pull unbounded state into every
/// trigger evaluation (ADR-0027 D-3). Like `reaction_eligibility` there is no
/// `accepts_global_storm` escape hatch on `action_definition`, so Global is Fatal.
pub const MF_VAL_REACTION_TRIGGER_GLOBAL: &str = "MF-VAL-239";

/// `turn_scheduler.scheduler_kind` legal values. `round_robin` is the value
/// this change ships; modder kinds arrive with kernel dispatch, not a Rust enum.
/// (Unlike `resets_on`, `scheduler_kind` is a substrate concept, not world
/// vocabulary — it is not the 5e-shape carrier CR-12 reshaped.)
const SCHEDULER_KIND_LEGAL: &[&str] = &["round_robin", "rolled_order"];

/// ADR-0037: `RolledOrder` requires a non-empty, parseable `order_roll_source`.
pub const MF_VAL_ROLLED_ORDER_SOURCE: &str = "MF-VAL-240";
/// ADR-0037: session_definition.turn_scheduler_id must FK-resolve when non-empty.
pub const MF_VAL_SESSION_DEF_SCHEDULER_FK: &str = "MF-VAL-241";
/// #1241 (Warning, soft guard): an `economy_channel_definitions` row with
/// `off_turn_eligible = true` AND `pool_ms_per_round = 0` is a dead class — its
/// budget is always exhausted, so the off-turn actions it marks can never fire.
/// Almost certainly an authoring error (an off-turn class needs ≥ 1 use).
pub const MF_VAL_OFF_TURN_ELIGIBLE_ZERO_POOL: &str = "MF-VAL-340";
/// A positive `session_definition.round_duration_ms` must land exactly on the
/// configured fiction-time tick quantum.
pub const MF_VAL_SESSION_DEF_ROUND_DURATION_QUANTIZATION: &str = "MF-VAL-342";
/// A boundary referenced by an economy channel or recharge has no kernel or
/// authored `FireBoundary` source that can raise it.
pub const MF_VAL_UNFIREABLE_BOUNDARY: &str = "MF-VAL-351";
/// A `FireBoundary` manifest payload is malformed or references a boundary
/// outside its consequence template bundle's dependency closure.
pub const MF_VAL_FIRE_BOUNDARY_REF: &str = "MF-VAL-353";
/// A boundary-armed autonomy job references an unknown or out-of-scope
/// `boundary_definitions` row.
pub const MF_VAL_AUTONOMY_JOB_BOUNDARY_REF: &str = "MF-VAL-355";
/// A session's combat-grid blueprint is missing from its visible dependency
/// closure or is declared by a bundle outside that closure.
pub const MF_VAL_COMBAT_GRID_BLUEPRINT_REF: &str = "MF-VAL-280";
pub const MF_VAL_COMBAT_GRID_BLUEPRINT_SHAPE: &str = "MF-VAL-281";
pub const MF_VAL_COMBAT_GRID_BLUEPRINT_CELL_SIZE: &str = "MF-VAL-282";
pub const MF_VAL_COMBAT_GRID_BLUEPRINT_TOPOLOGY: &str = "MF-VAL-283";
pub const MF_VAL_COMBAT_GRID_BLUEPRINT_AFFORDANCE_REF: &str = "MF-VAL-284";
pub const MF_VAL_COMBAT_GRID_BLUEPRINT_BINDINGS_JSON: &str = "MF-VAL-285";
pub const MF_VAL_COMBAT_GRID_BLUEPRINT_OVERRIDES_JSON: &str = "MF-VAL-286";
pub const MF_VAL_COMBAT_GRID_BLUEPRINT_OVERRIDE_SHAPE: &str = "MF-VAL-287";
pub const MF_VAL_COMBAT_GRID_BLUEPRINT_OVERRIDE_ENDPOINTS: &str = "MF-VAL-288";
pub const MF_VAL_COMBAT_GRID_BLUEPRINT_OVERRIDE_BINDINGS: &str = "MF-VAL-289";
pub const MF_VAL_COMBAT_GRID_BLUEPRINT_OVERRIDE_COST: &str = "MF-VAL-290";
/// ADR-0134 D-1: a blueprint's `distance_metric_id` does not resolve to a
/// visible `distance_metric_definitions` row (the three `core/metric/*`
/// seeds are always visible). An unknown id is an authoring error, never a
/// silent Euclidean fallthrough — the same posture `is_known_area_shape_id`
/// takes for ADR-0042 D-3.
pub const MF_VAL_COMBAT_GRID_BLUEPRINT_DISTANCE_METRIC_REF: &str = "MF-VAL-332";
/// ADR-0134 D-3 compensating control: the kernel no longer vetoes a step
/// onto an occupied cell, so a blueprint whose `edge_affordance_template_id`
/// template reads no `occupants_at(...)` lets actors stack. That is refused
/// unless the blueprint states the intent with
/// `accepts_stacked_occupancy = true` (the ADR-0027 D-1 `accepts_global_storm`
/// pattern). A template this manifest does not declare cannot be inspected
/// and counts as reading nothing — fail closed, never trust.
pub const MF_VAL_COMBAT_GRID_BLUEPRINT_STACKED_OCCUPANCY: &str = "MF-VAL-333";
/// ADR-0143 D-1: a nonblank `session_definition.turn_eligibility_pred` must
/// parse as a Bool predicate (macros expanded) and read only what the
/// per-candidate evaluation context backs with data
/// ([`turn_eligibility_admits`]). Every failure is Fatal.
pub const MF_VAL_SESSION_DEF_TURN_ELIGIBILITY: &str = "MF-VAL-334";

/// Validate the action-economy authoring sections. Slice A landed the
/// `economy_channel_definitions` + `turn_scheduler` sections; slice C adds the
/// `affordance_template` budget/recharge field FK + boundary resolution.
pub fn validate_action_economy(manifest: &Manifest, diags: &mut Vec<Diagnostic>) {
    validate_economy_channel_definitions(manifest, diags);
    let macros = crate::dsl_macros::dsl_macro_compiler_definitions(manifest);
    validate_turn_scheduler(manifest, &macros, diags);
    validate_session_definitions(manifest, diags);
    validate_session_turn_eligibility(manifest, &macros, diags);
    validate_combat_grid_blueprints(manifest, diags);
    validate_affordance_template_action_economy(manifest, diags);
    validate_boundary_fireability(manifest, diags);
    validate_reaction_eligibility(manifest, &macros, diags);
    validate_reaction_trigger(manifest, &macros, diags);
}

const KERNEL_FIRED_BOUNDARIES: &[&str] = &["turn_start", "turn_end"];

#[derive(Clone, Copy)]
struct FireBoundarySource<'a> {
    boundary_id: &'a str,
    bundle_id: &'a str,
}

/// D-4's fireable-or-rejected rule. A boundary reference is live only when
/// the turn kernel raises it itself or an in-scope authored consequence can
/// raise it through `FrameWriteOp::FireBoundary`. The complete reference set
/// is channel `resets_on`, affordance `recharges_on`, boundary-relative
/// condition expiry, and boundary-armed autonomy jobs.
fn validate_boundary_fireability(manifest: &Manifest, diags: &mut Vec<Diagnostic>) {
    let scope = BundleScope::build(manifest);
    let present: BTreeSet<&str> = manifest
        .bundles
        .iter()
        .map(|b| b.bundle_id.as_str())
        .collect();
    let boundary_bundle: BTreeMap<&str, &str> = manifest
        .boundary_definitions
        .iter()
        .map(|b| (b.id.as_str(), b.bundle_id.as_str()))
        .collect();
    let template_bundle: BTreeMap<&str, &str> = manifest
        .consequence_templates
        .iter()
        .map(|t| (t.template_id.as_str(), t.bundle_id.as_str()))
        .collect();

    let mut sources = Vec::new();
    let mut expiry_refs = Vec::new();
    for row in &manifest.consequence_frame_writes {
        let bundle_id = template_bundle
            .get(row.template_id.as_str())
            .copied()
            .unwrap_or("");
        collect_fire_boundary_sources(&row.frame_write, bundle_id, &mut sources, diags);
        collect_boundary_expiry_refs(&row.frame_write, bundle_id, &mut expiry_refs, diags);
    }

    // A malformed/out-of-scope FireBoundary must not make another row appear
    // fireable. The source itself is checked with the same scoped-reference
    // rule as the consumer fields.
    let valid_sources: Vec<_> = sources
        .into_iter()
        .filter(|source| {
            match resolve_scoped_ref(
                source.bundle_id,
                source.boundary_id,
                &boundary_bundle,
                &scope,
                &present,
            ) {
                RefResolution::InScope | RefResolution::Trusted => true,
                RefResolution::OutOfScope { decl_bundle } => {
                    diags.push(Diagnostic {
                        code: MF_VAL_FIRE_BOUNDARY_REF.to_owned(),
                        severity: Severity::Fatal,
                        message: format!(
                            "FireBoundary boundary_id '{}' is declared by bundle '{decl_bundle}', \
                             which is not in '{}'s depends_on closure",
                            source.boundary_id, source.bundle_id
                        ),
                        path: "consequence_frame_writes.frame_write.FireBoundary.boundary_id".into(),
                        span: None,
                        hint: Some(format!(
                            "add '{decl_bundle}' to the consequence bundle's depends_on, or fire an in-scope boundary"
                        )),
                        details: None,
                    });
                    false
                }
                RefResolution::UndeclaredButVisible => {
                    diags.push(Diagnostic {
                        code: MF_VAL_FIRE_BOUNDARY_REF.to_owned(),
                        severity: Severity::Fatal,
                        message: format!(
                            "FireBoundary boundary_id '{}' does not resolve to an authored boundary_definitions row",
                            source.boundary_id
                        ),
                        path: "consequence_frame_writes.frame_write.FireBoundary.boundary_id".into(),
                        span: None,
                        hint: Some("author the boundary_definitions row before firing it".into()),
                        details: None,
                    });
                    false
                }
            }
        })
        .collect();

    let mut autonomy_boundary_refs = Vec::new();
    for row in &manifest.autonomy_jobs {
        let boundary = row.fire_at_boundary.trim();
        if row.next_fire_at_ms.is_some() || boundary.is_empty() || row.owner_actor == 0 {
            continue;
        }
        match resolve_scoped_ref(&row.bundle_id, boundary, &boundary_bundle, &scope, &present) {
            RefResolution::InScope => {
                autonomy_boundary_refs.push((
                    boundary,
                    row.bundle_id.as_str(),
                    row.job_id.as_str(),
                ));
            }
            RefResolution::Trusted => {}
            RefResolution::OutOfScope { decl_bundle } => diags.push(Diagnostic {
                code: MF_VAL_AUTONOMY_JOB_BOUNDARY_REF.to_owned(),
                severity: Severity::Fatal,
                message: format!(
                    "autonomy_job '{}' fire_at_boundary '{}' is declared by bundle \
                     '{decl_bundle}', which is not in '{}'s depends_on closure",
                    row.job_id, boundary, row.bundle_id
                ),
                path: format!("autonomy_jobs[{}].fire_at_boundary", row.job_id),
                span: None,
                hint: Some(format!(
                    "add '{decl_bundle}' to this bundle's depends_on, or arm an in-scope boundary"
                )),
                details: None,
            }),
            RefResolution::UndeclaredButVisible => diags.push(Diagnostic {
                code: MF_VAL_AUTONOMY_JOB_BOUNDARY_REF.to_owned(),
                severity: Severity::Fatal,
                message: format!(
                    "autonomy_job '{}' fire_at_boundary '{}' does not resolve to an authored \
                     boundary_definitions row",
                    row.job_id, boundary
                ),
                path: format!("autonomy_jobs[{}].fire_at_boundary", row.job_id),
                span: None,
                hint: Some(
                    "author the boundary_definitions row in this bundle or a dependency".into(),
                ),
                details: None,
            }),
        }
    }

    let mut validate_reference = |boundary: &str, bundle_id: &str, path: String| {
        if !matches!(
            resolve_scoped_ref(bundle_id, boundary, &boundary_bundle, &scope, &present),
            RefResolution::InScope
        ) {
            // The existing FK validators own out-of-scope/unknown errors; an
            // absent dependency stays trusted per the manifest-fragment rule.
            return;
        }
        let kernel_fired = KERNEL_FIRED_BOUNDARIES.contains(&boundary);
        let authored_source = valid_sources.iter().any(|source| {
            source.boundary_id == boundary && scope.in_scope(bundle_id, source.bundle_id)
        });
        if !kernel_fired && !authored_source {
            diags.push(Diagnostic {
                code: MF_VAL_UNFIREABLE_BOUNDARY.to_owned(),
                severity: Severity::Fatal,
                message: format!(
                    "boundary '{boundary}' is referenced by '{bundle_id}' but no kernel or \
                     in-scope FireBoundary consequence can raise it"
                ),
                path,
                span: None,
                hint: Some(
                    "author a consequence_frame_writes FireBoundary { boundary_id, scope } in this bundle or a dependency"
                        .into(),
                ),
                details: None,
            });
        }
    };

    for row in &manifest.economy_channel_definitions {
        validate_reference(
            &row.resets_on,
            &row.bundle_id,
            format!("economy_channel_definitions[{}].resets_on", row.id),
        );
    }
    for row in &manifest.affordance_templates {
        if let Some(boundary) = row.recharges_on.as_deref().filter(|s| !s.is_empty()) {
            validate_reference(
                boundary,
                &row.bundle_id,
                format!("affordance_templates[{}].recharges_on", row.template_id),
            );
        }
    }
    for (boundary, bundle_id, job_id) in autonomy_boundary_refs {
        validate_reference(
            boundary,
            bundle_id,
            format!("autonomy_jobs[{job_id}].fire_at_boundary"),
        );
    }
    for (boundary, bundle_id) in expiry_refs {
        validate_reference(
            boundary,
            bundle_id,
            "consequence_frame_writes.frame_write.ApplyStatus.expires_at_boundary.boundary_id"
                .into(),
        );
    }
}

/// Boundary-relative status expiry is a consumer of the same authored boundary
/// vocabulary as recharges. Recurse only through the one admitted compound
/// frame-write carrier so validation stays structural and content-neutral.
fn collect_boundary_expiry_refs<'a>(
    value: &'a serde_json::Value,
    bundle_id: &'a str,
    out: &mut Vec<(&'a str, &'a str)>,
    diags: &mut Vec<Diagnostic>,
) {
    let Some(map) = value.as_object() else {
        return;
    };
    if let Some(payload) = map
        .get("ApplyStatus")
        .and_then(serde_json::Value::as_object)
    {
        if let Some(expiry) = payload.get("expires_at_boundary") {
            let boundary = expiry
                .as_object()
                .and_then(|fields| fields.get("boundary_id"))
                .and_then(serde_json::Value::as_str)
                .filter(|value| !value.trim().is_empty());
            if let Some(boundary) = boundary {
                out.push((boundary, bundle_id));
            } else {
                diags.push(fire_boundary_shape_diag(
                    "ApplyStatus expires_at_boundary requires a non-empty boundary_id",
                ));
            }
        }
    }
    if let Some(body) = map
        .get("ForEachActorInRadius")
        .and_then(serde_json::Value::as_object)
        .and_then(|fields| fields.get("body"))
        .and_then(serde_json::Value::as_array)
    {
        for child in body {
            collect_boundary_expiry_refs(child, bundle_id, out, diags);
        }
    }
}

fn collect_fire_boundary_sources<'a>(
    value: &'a serde_json::Value,
    bundle_id: &'a str,
    out: &mut Vec<FireBoundarySource<'a>>,
    diags: &mut Vec<Diagnostic>,
) {
    let Some(map) = value.as_object() else {
        return;
    };
    if let Some(payload) = map.get("FireBoundary") {
        let Some(fields) = payload.as_object() else {
            diags.push(fire_boundary_shape_diag(
                "FireBoundary payload must be an object",
            ));
            return;
        };
        let boundary_id = fields
            .get("boundary_id")
            .and_then(serde_json::Value::as_str);
        let scope = fields.get("scope").and_then(serde_json::Value::as_str);
        match (
            boundary_id.filter(|s| !s.trim().is_empty()),
            scope.filter(|s| !s.trim().is_empty()),
        ) {
            (Some(boundary_id), Some(_)) => out.push(FireBoundarySource {
                boundary_id,
                bundle_id,
            }),
            _ => diags.push(fire_boundary_shape_diag(
                "FireBoundary requires non-empty string boundary_id and scope fields",
            )),
        }
        return;
    }
    if let Some(body) = map
        .get("ForEachActorInRadius")
        .and_then(serde_json::Value::as_object)
        .and_then(|fields| fields.get("body"))
        .and_then(serde_json::Value::as_array)
    {
        for child in body {
            collect_fire_boundary_sources(child, bundle_id, out, diags);
        }
    }
}

fn fire_boundary_shape_diag(message: &str) -> Diagnostic {
    Diagnostic {
        code: MF_VAL_FIRE_BOUNDARY_REF.to_owned(),
        severity: Severity::Fatal,
        message: message.to_owned(),
        path: "consequence_frame_writes.frame_write.FireBoundary".into(),
        span: None,
        hint: Some("use { FireBoundary = { boundary_id = \"...\", scope = \"Actor\" } }".into()),
        details: None,
    }
}

/// Slice E (warden condition 6): an `action_definition.reaction_eligibility`
/// predicate is evaluated by the reaction producer against candidate reactors
/// on every triggering layer-2 event. It MUST be locality-bounded — a Global
/// predicate would scan unbounded world state per event. There is no
/// `accepts_global_storm` opt-in on `action_definition` (reactions, unlike
/// affordances, never run a deliberate world-storm sweep), so Global is Fatal.
/// A parse failure reuses MF-VAL-201 like every other predicate surface.
fn validate_reaction_eligibility(
    manifest: &Manifest,
    macros: &[spellkave_dsl::DslMacroDefinition],
    diags: &mut Vec<Diagnostic>,
) {
    use crate::locality::{self, LocalityClass};
    use crate::MF_VAL_PREDICATE_PARSE;

    for row in &manifest.action_definitions {
        let Some(source) = row.reaction_eligibility.as_deref() else {
            continue;
        };
        if source.trim().is_empty() {
            continue;
        }

        let expr = match crate::dsl_macros::parse_predicate(source, macros) {
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
                        "action_definition '{}': reaction_eligibility parse error: {msg}",
                        row.action_id
                    ),
                    path: format!("action_definitions[{}].reaction_eligibility", row.action_id),
                    span: None,
                    hint: None,
                    details: Some(serde_json::json!({ "action_id": row.action_id })),
                });
                continue;
            }
        };

        if locality::classify(&expr) == LocalityClass::Global {
            diags.push(Diagnostic {
                code: MF_VAL_REACTION_ELIGIBILITY_GLOBAL.to_owned(),
                severity: Severity::Fatal,
                message: format!(
                    "action_definition '{}': reaction_eligibility classifies as Global \
                     locality; a reaction predicate must be bounded (Local/Place/Family) \
                     so the producer's per-event candidate scan stays bounded (ADR-0027 D-3)",
                    row.action_id
                ),
                path: format!("action_definitions[{}].reaction_eligibility", row.action_id),
                span: None,
                hint: Some(
                    "narrow the predicate to the reactor's own fields/capabilities, the \
                     place node, or a tagged family — not world.* / session state"
                        .to_owned(),
                ),
                details: Some(serde_json::json!({
                    "action_id": row.action_id,
                    "locality_class": "Global",
                })),
            });
        }
    }
}

/// Slice F-impl (warden NS-2): an `action_definition.reaction_trigger` predicate
/// (§8.4 WHAT-provokes) is evaluated by the reaction producer ONCE per triggering
/// event against the fixed intent facts (`$event.kind`, `$event.detail.*`). The
/// `$event.*` surface is bounded — `classify_reaction_trigger` treats it Local —
/// but a `world.*`/`session.*` read would pull unbounded state into every event,
/// so a Global-classified trigger is Fatal (no `accepts_global_storm` opt-in on
/// `action_definition`, mirroring `reaction_eligibility`/MF-VAL-238). A parse
/// failure reuses MF-VAL-201.
fn validate_reaction_trigger(
    manifest: &Manifest,
    macros: &[spellkave_dsl::DslMacroDefinition],
    diags: &mut Vec<Diagnostic>,
) {
    use crate::locality::{self, LocalityClass};
    use crate::MF_VAL_PREDICATE_PARSE;

    for row in &manifest.action_definitions {
        let Some(source) = row.reaction_trigger.as_deref() else {
            continue;
        };
        if source.trim().is_empty() {
            continue;
        }

        let expr = match crate::dsl_macros::parse_predicate(source, macros) {
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
                        "action_definition '{}': reaction_trigger parse error: {msg}",
                        row.action_id
                    ),
                    path: format!("action_definitions[{}].reaction_trigger", row.action_id),
                    span: None,
                    hint: None,
                    details: Some(serde_json::json!({ "action_id": row.action_id })),
                });
                continue;
            }
        };

        if locality::classify_reaction_trigger(&expr) == LocalityClass::Global {
            diags.push(Diagnostic {
                code: MF_VAL_REACTION_TRIGGER_GLOBAL.to_owned(),
                severity: Severity::Fatal,
                message: format!(
                    "action_definition '{}': reaction_trigger classifies as Global \
                     locality; a trigger predicate may read the event (`$event.*`) and \
                     the reactor's own fields, not unbounded world/session state, so the \
                     producer's per-event evaluation stays bounded (ADR-0027 D-3)",
                    row.action_id
                ),
                path: format!("action_definitions[{}].reaction_trigger", row.action_id),
                span: None,
                hint: Some(
                    "scope the trigger to `$event.kind` / `$event.detail.*` (and the \
                     reactor's own fields) — not world.* / session state"
                        .to_owned(),
                ),
                details: Some(serde_json::json!({
                    "action_id": row.action_id,
                    "locality_class": "Global",
                })),
            });
        }
    }
}

fn validate_economy_channel_definitions(manifest: &Manifest, diags: &mut Vec<Diagnostic>) {
    let scope = BundleScope::build(manifest);
    // Authored reset-boundary vocabulary: token id -> declaring bundle_id.
    let boundary_bundle: BTreeMap<&str, &str> = manifest
        .boundary_definitions
        .iter()
        .map(|b| (b.id.as_str(), b.bundle_id.as_str()))
        .collect();
    // Bundles whose `[[bundles]]` row is present in this manifest. Empty for a
    // bare rows file (no topology) — then the existence tier never fires.
    let present: BTreeSet<&str> = manifest
        .bundles
        .iter()
        .map(|b| b.bundle_id.as_str())
        .collect();

    let mut seen: BTreeSet<&str> = BTreeSet::new();
    for row in &manifest.economy_channel_definitions {
        if !seen.insert(row.id.as_str()) {
            diags.push(Diagnostic {
                code: MF_VAL_CHANNEL_DUP.to_owned(),
                severity: Severity::Fatal,
                message: format!("duplicate economy_channel_definitions id '{}'", row.id),
                path: format!("economy_channel_definitions[{}]", row.id),
                span: None,
                hint: Some("each action class id must be unique".to_owned()),
                details: None,
            });
        }
        validate_resets_on(row, &scope, &boundary_bundle, &present, diags);
        // #1241 soft guard: an off-turn-eligible class with zero capacity is a
        // dead class (always exhausted) — its off-turn actions can never fire.
        if row.off_turn_eligible && row.pool_ms_per_round == 0 {
            diags.push(Diagnostic {
                code: MF_VAL_OFF_TURN_ELIGIBLE_ZERO_POOL.to_owned(),
                severity: Severity::Warning,
                message: format!(
                    "economy_channel_definitions '{}' is off_turn_eligible but has \
                     pool_ms_per_round = 0 (always exhausted — off-turn actions can never fire)",
                    row.id
                ),
                path: format!("economy_channel_definitions[{}].pool_ms_per_round", row.id),
                span: None,
                hint: Some(
                    "give an off-turn-eligible class a pool_ms_per_round of at least 1 \
                     (reactions are typically capacity 1, resets_on = turn_start)"
                        .to_owned(),
                ),
                details: None,
            });
        }
    }
}

/// Resolve one `economy_channel_definitions.resets_on` against authored
/// `boundary_definitions`, scoped by `depends_on` closure. See the module
/// doc for the two-tier rule (scope / existence).
fn validate_resets_on(
    row: &EconomyChannelDefinitionRow,
    scope: &BundleScope,
    boundary_bundle: &BTreeMap<&str, &str>,
    present: &BTreeSet<&str>,
    diags: &mut Vec<Diagnostic>,
) {
    match resolve_scoped_ref(
        &row.bundle_id,
        &row.resets_on,
        boundary_bundle,
        scope,
        present,
    ) {
        RefResolution::InScope | RefResolution::Trusted => {}
        RefResolution::OutOfScope { decl_bundle } => {
            diags.push(Diagnostic {
                code: MF_VAL_CHANNEL_RESETS_ON.to_owned(),
                severity: Severity::Fatal,
                message: format!(
                    "economy_channel_definitions '{}' resets_on '{}' is declared by bundle \
                     '{decl_bundle}', which is not in '{}'s depends_on closure",
                    row.id, row.resets_on, row.bundle_id
                ),
                path: format!("economy_channel_definitions[{}].resets_on", row.id),
                span: None,
                hint: Some(format!(
                    "add '{decl_bundle}' to this bundle's depends_on, or declare the \
                     boundary_definitions row in an in-scope bundle"
                )),
                details: None,
            });
        }
        RefResolution::UndeclaredButVisible => {
            diags.push(Diagnostic {
                code: MF_VAL_CHANNEL_RESETS_ON.to_owned(),
                severity: Severity::Fatal,
                message: format!(
                    "economy_channel_definitions '{}' resets_on '{}' does not resolve to any \
                     in-scope boundary_definitions row",
                    row.id, row.resets_on
                ),
                path: format!("economy_channel_definitions[{}].resets_on", row.id),
                span: None,
                hint: Some(
                    "author a boundary_definitions row for this boundary (id = the \
                     token), in this bundle or a depended-on one"
                        .to_owned(),
                ),
                details: None,
            });
        }
    }
}

/// Slice C: validate the two `affordance_template` action-economy fields that
/// the budget gate + recharge clock read. `spends_channel` is an FK into
/// `economy_channel_definitions`; `recharges_on` resolves against authored
/// `boundary_definitions` (the same vocabulary `resets_on` uses — warden
/// condition 7). Both are scoped by the affordance template's `depends_on`
/// closure and stay lenient when a dependency's declarations are absent.
fn validate_affordance_template_action_economy(manifest: &Manifest, diags: &mut Vec<Diagnostic>) {
    // Nothing authored → nothing to resolve (the common case — most affordances
    // are outside the action economy).
    let any_field = manifest
        .affordance_templates
        .iter()
        .any(|t| t.spends_channel.is_some() || t.recharges_on.is_some() || t.composes.is_some());
    if !any_field {
        return;
    }

    let scope = BundleScope::build(manifest);
    let present: BTreeSet<&str> = manifest
        .bundles
        .iter()
        .map(|b| b.bundle_id.as_str())
        .collect();
    let class_bundle: BTreeMap<&str, &str> = manifest
        .economy_channel_definitions
        .iter()
        .map(|c| (c.id.as_str(), c.bundle_id.as_str()))
        .collect();
    let boundary_bundle: BTreeMap<&str, &str> = manifest
        .boundary_definitions
        .iter()
        .map(|b| (b.id.as_str(), b.bundle_id.as_str()))
        .collect();
    // Composition FK target space (slice D): every authored affordance_template
    // is a legal `composes` sub-id, scoped by the using template's depends_on
    // closure (same two-tier rule).
    let template_bundle: BTreeMap<&str, &str> = manifest
        .affordance_templates
        .iter()
        .map(|t| (t.template_id.as_str(), t.bundle_id.as_str()))
        .collect();
    // Composition is single-level (LR-7): a wrapper's children must be leaf
    // actions. `wrapper_ids` is the set of PRESENT templates that carry a
    // non-empty `composes` — a child in this set is itself a wrapper, which the
    // single-level unroll would silently mis-execute. Built from present
    // templates only, so an absent child (wrapper-status unknowable) is naturally
    // trusted, matching the FK's lenient-when-dependency-absent posture.
    let wrapper_ids: BTreeSet<&str> = manifest
        .affordance_templates
        .iter()
        .filter(|t| {
            t.composes
                .as_deref()
                .unwrap_or(&[])
                .iter()
                .any(|s| !s.is_empty())
        })
        .map(|t| t.template_id.as_str())
        .collect();

    for t in &manifest.affordance_templates {
        if let Some(class_id) = t.spends_channel.as_deref().filter(|s| !s.is_empty()) {
            match resolve_scoped_ref(&t.bundle_id, class_id, &class_bundle, &scope, &present) {
                RefResolution::InScope | RefResolution::Trusted => {}
                RefResolution::OutOfScope { decl_bundle } => diags.push(Diagnostic {
                    code: MF_VAL_SPENDS_CHANNEL.to_owned(),
                    severity: Severity::Fatal,
                    message: format!(
                        "affordance_template '{}' spends_channel '{class_id}' is declared \
                         by bundle '{decl_bundle}', which is not in '{}'s depends_on closure",
                        t.template_id, t.bundle_id
                    ),
                    path: format!("affordance_templates[{}].spends_channel", t.template_id),
                    span: None,
                    hint: Some(format!(
                        "add '{decl_bundle}' to this bundle's depends_on, or use an in-scope \
                         economy_channel_definitions id"
                    )),
                    details: None,
                }),
                RefResolution::UndeclaredButVisible => diags.push(Diagnostic {
                    code: MF_VAL_SPENDS_CHANNEL.to_owned(),
                    severity: Severity::Fatal,
                    message: format!(
                        "affordance_template '{}' spends_channel '{class_id}' does not \
                         resolve to any in-scope economy_channel_definitions row",
                        t.template_id
                    ),
                    path: format!("affordance_templates[{}].spends_channel", t.template_id),
                    span: None,
                    hint: Some(
                        "author an economy_channel_definitions row with this id, or reference an \
                         existing class (e.g. core/action_class/primary)"
                            .to_owned(),
                    ),
                    details: None,
                }),
            }
        }

        if let Some(boundary) = t.recharges_on.as_deref().filter(|s| !s.is_empty()) {
            match resolve_scoped_ref(&t.bundle_id, boundary, &boundary_bundle, &scope, &present) {
                RefResolution::InScope | RefResolution::Trusted => {}
                RefResolution::OutOfScope { decl_bundle } => diags.push(Diagnostic {
                    code: MF_VAL_RECHARGES_ON.to_owned(),
                    severity: Severity::Fatal,
                    message: format!(
                        "affordance_template '{}' recharges_on '{boundary}' is declared by bundle \
                         '{decl_bundle}', which is not in '{}'s depends_on closure",
                        t.template_id, t.bundle_id
                    ),
                    path: format!("affordance_templates[{}].recharges_on", t.template_id),
                    span: None,
                    hint: Some(format!(
                        "add '{decl_bundle}' to this bundle's depends_on, or declare the \
                         boundary_definitions row in an in-scope bundle"
                    )),
                    details: None,
                }),
                RefResolution::UndeclaredButVisible => diags.push(Diagnostic {
                    code: MF_VAL_RECHARGES_ON.to_owned(),
                    severity: Severity::Fatal,
                    message: format!(
                        "affordance_template '{}' recharges_on '{boundary}' does not resolve to \
                         any in-scope boundary_definitions row",
                        t.template_id
                    ),
                    path: format!("affordance_templates[{}].recharges_on", t.template_id),
                    span: None,
                    hint: Some(
                        "recharge on an authored boundary_definitions id (turn advancement \
                         raises turn_start / turn_end)"
                            .to_owned(),
                    ),
                    details: None,
                }),
            }
        }

        // Composition FK (slice D): each `composes` sub-id must resolve to an
        // in-scope affordance_template. Same two-tier scope/existence rule.
        if let Some(children) = t.composes.as_ref() {
            for child in children.iter().filter(|s| !s.is_empty()) {
                match resolve_scoped_ref(&t.bundle_id, child, &template_bundle, &scope, &present) {
                    RefResolution::InScope | RefResolution::Trusted => {}
                    RefResolution::OutOfScope { decl_bundle } => diags.push(Diagnostic {
                        code: MF_VAL_COMPOSES_REF.to_owned(),
                        severity: Severity::Fatal,
                        message: format!(
                            "affordance_template '{}' composes '{child}' is declared by bundle \
                             '{decl_bundle}', which is not in '{}'s depends_on closure",
                            t.template_id, t.bundle_id
                        ),
                        path: format!("affordance_templates[{}].composes", t.template_id),
                        span: None,
                        hint: Some(format!(
                            "add '{decl_bundle}' to this bundle's depends_on, or compose an \
                             in-scope affordance_template"
                        )),
                        details: None,
                    }),
                    RefResolution::UndeclaredButVisible => diags.push(Diagnostic {
                        code: MF_VAL_COMPOSES_REF.to_owned(),
                        severity: Severity::Fatal,
                        message: format!(
                            "affordance_template '{}' composes '{child}' does not resolve to any \
                             in-scope affordance_template",
                            t.template_id
                        ),
                        path: format!("affordance_templates[{}].composes", t.template_id),
                        span: None,
                        hint: Some(
                            "compose only authored affordance_template ids (the sub-attacks of a \
                             Multiattack are ordinary affordance rows)"
                                .to_owned(),
                        ),
                        details: None,
                    }),
                }
                // Single-level (LR-7): a child that is itself a composition
                // wrapper would silently mis-execute under the single-level
                // unroll — its own outcome frame fires and its grandchildren
                // never run. `wrapper_ids` holds only PRESENT wrappers, so an
                // absent child is trusted (same lenient posture as the FK above).
                if wrapper_ids.contains(child.as_str()) {
                    diags.push(Diagnostic {
                        code: MF_VAL_COMPOSES_NESTED.to_owned(),
                        severity: Severity::Fatal,
                        message: format!(
                            "affordance_template '{}' composes '{child}', which is itself a \
                             composition wrapper — composition is single-level (a wrapper's \
                             children must be leaf actions)",
                            t.template_id
                        ),
                        path: format!("affordance_templates[{}].composes", t.template_id),
                        span: None,
                        hint: Some(
                            "compose only leaf affordance_templates (the sub-attacks of a \
                             Multiattack are ordinary, non-composing rows); flatten the nesting"
                                .to_owned(),
                        ),
                        details: None,
                    });
                }
            }
        }
    }
}

fn validate_turn_scheduler(
    manifest: &Manifest,
    macros: &[spellkave_dsl::DslMacroDefinition],
    diags: &mut Vec<Diagnostic>,
) {
    let mut active_ids: Vec<&str> = Vec::new();
    for row in &manifest.turn_scheduler {
        if !SCHEDULER_KIND_LEGAL.contains(&row.scheduler_kind.as_str()) {
            diags.push(Diagnostic {
                code: MF_VAL_TURN_SCHEDULER_KIND.to_owned(),
                severity: Severity::Fatal,
                message: format!(
                    "turn_scheduler '{}' scheduler_kind '{}' is not one of {}",
                    row.id,
                    row.scheduler_kind,
                    SCHEDULER_KIND_LEGAL.join("|")
                ),
                path: format!("turn_scheduler[{}].scheduler_kind", row.id),
                span: None,
                hint: Some(format!("use one of: {}", SCHEDULER_KIND_LEGAL.join(", "))),
                details: None,
            });
        }
        if row.active {
            active_ids.push(row.id.as_str());
        }
    }
    // The "exactly one active scheduler per world" invariant has two arms.
    // Multi-active is catchable here (two active rows authored in one manifest).
    // Zero-active is NOT — a manifest *fragment* (a non-core bundle) legitimately
    // authors no scheduler and relies on core's; the zero-active-per-world arm is
    // a runtime invariant (r_advance_turn finds no active scheduler — slice B's
    // scheduler-row-absence K-5 arm), not a single-manifest validation.
    if active_ids.len() > 1 {
        diags.push(Diagnostic {
            code: MF_VAL_TURN_SCHEDULER_ACTIVE.to_owned(),
            severity: Severity::Fatal,
            message: format!(
                "multiple turn_scheduler rows active in one manifest: {}",
                active_ids.join(", ")
            ),
            path: "turn_scheduler".to_owned(),
            span: None,
            hint: Some(
                "exactly one turn_scheduler row may be active; disable the others".to_owned(),
            ),
            details: None,
        });
    }

    // ADR-0037: RolledOrder requires a non-empty, parseable order_roll_source.
    for row in &manifest.turn_scheduler {
        if row.scheduler_kind == "rolled_order" {
            let src = row.order_roll_source.as_deref().unwrap_or("");
            if src.is_empty() {
                diags.push(Diagnostic {
                    code: MF_VAL_ROLLED_ORDER_SOURCE.to_owned(),
                    severity: Severity::Fatal,
                    message: format!(
                        "turn_scheduler '{}' scheduler_kind='rolled_order' requires non-empty order_roll_source",
                        row.id
                    ),
                    path: format!("turn_scheduler[{}].order_roll_source", row.id),
                    span: None,
                    hint: Some("set order_roll_source to a dice expression such as \"d20\"".to_owned()),
                    details: None,
                });
            } else if !order_roll_source_is_valid(src, macros) {
                diags.push(Diagnostic {
                    code: MF_VAL_ROLLED_ORDER_SOURCE.to_owned(),
                    severity: Severity::Fatal,
                    message: format!(
                        "turn_scheduler '{}' order_roll_source '{src}' is not a valid initiative roll expression",
                        row.id
                    ),
                    path: format!("turn_scheduler[{}].order_roll_source", row.id),
                    span: None,
                    hint: Some(
                        "an authored dice/int DSL expression evaluated once per participant, e.g. d20 or 2d6".to_owned(),
                    ),
                    details: None,
                });
            }
        }
    }
}

/// Validates an initiative integer expression (ADR-0037). Only facts for
/// the rolling actor are available. Actor field reads can be checked for
/// shape before their values are available.
fn order_roll_source_is_valid(src: &str, macros: &[spellkave_dsl::DslMacroDefinition]) -> bool {
    let Ok(expr) = crate::dsl_macros::parse_int_expression(src, macros) else {
        return false;
    };
    if !only_reads_actor_subject(&expr) {
        return false;
    }
    let facts = spellkave_dsl::EvaluationContext::default();
    let limits = spellkave_dsl::EvaluationLimits::default();
    let mut rng = spellkave_dsl::DeterministicRng::new(vec![0; limits.max_draws as usize]);
    match spellkave_dsl::evaluate_int_expression(&expr, &facts, &mut rng, limits) {
        Ok(_) => true,
        // Finding #26: an Actor-subject `int`/`effective_int` field is
        // populated per-participant at RUNTIME (`resolve_initiative`), never
        // known ahead of time — accept this ONE error shape (narrowed to
        // ReadInt/ReadEffectiveInt by construction, see the doc comment
        // above). Any other evaluation error (bad roll_table id, division
        // shape, unknown binding, …) still fails validation.
        Err(spellkave_dsl::EvaluationError::UnknownField {
            subject: spellkave_dsl::SubjectRef::Actor,
            ..
        }) => true,
        Err(_) => false,
    }
}

/// Checks whether the expression can run with facts for a single actor.
/// Match every operator explicitly so additions require a classification.
fn only_reads_actor_subject(expr: &spellkave_dsl::Expression) -> bool {
    use spellkave_dsl::{Op, SubjectRef};
    expr.nodes.iter().all(|node| match &node.op {
        // #30 / sk-ct8.2: `resource_max`/`resource_min` ride the SAME
        // `collect_entity_state_facts` populator as `ReadInt`/
        // `ReadEffectiveInt` (Actor-subject `resource_bounds` IS populated at
        // this call site) — same allow rule.
        Op::ReadInt(rf)
        | Op::ReadEffectiveInt(rf)
        | Op::ReadResourceMax(rf)
        | Op::ReadResourceMin(rf) => rf.subject == SubjectRef::Actor,
        Op::ReadBool(_) | Op::ReadText(_) => false,
        Op::HasCapability(h) => h.subject == SubjectRef::Actor,
        Op::HasTag(h) => h.subject == SubjectRef::Actor,
        Op::HasStatus(h) => h.subject == SubjectRef::Actor,
        Op::HasNiche(h) => h.subject == SubjectRef::Actor,
        Op::HasItem(h) => h.subject == SubjectRef::Actor,
        Op::TracePresent(_)
        | Op::KnowsEntity(_)
        | Op::SharesTag(_)
        | Op::ModifyResource(_)
        | Op::ApplyStatus(_)
        | Op::EmitTrace(_) => false,
        // The initiative context does not supply `peers_near_target` facts.
        Op::PeersNearTarget(_) => false,
        // ADR-0134 D-3: `occupants_at` needs a kernel-populated node scope
        // (`to_node` / `from_node` / `target`) that `resolve_initiative`'s
        // per-participant context never populates — GUARANTEED failure, same
        // posture as `PeersNearTarget` above, rejected outright.
        Op::OccupantsAt(_) => false,
        // sk-84i.2 (widened G-1): `<subject>.source(slot)` evaluates a
        // DATA-DEFINED sub-expression (an authored source text) this static
        // walk cannot see into — it might read ANY subject, not just Actor,
        // regardless of what THIS node's own `subject` is.
        // `resolve_initiative`'s per-participant context never populates
        // `item_dsl_sources` at all (same posture as Bool/Text above: a
        // GUARANTEED failure here — `NoItemDslSourceFacts`, not a
        // "wrong subject" risk) — rejected outright, same bucket as
        // TracePresent/KnowsEntity/etc.
        Op::DslSource(_) => false,
        Op::BoolLiteral(_)
        | Op::IntLiteral(_)
        | Op::TextLiteral(_)
        | Op::And(_)
        | Op::Or(_)
        | Op::Not(_)
        | Op::Compare(_)
        | Op::Arithmetic(_)
        | Op::Cond(_)
        | Op::Roll(_)
        | Op::RollUnder(_)
        | Op::RollTable(_)
        | Op::Let(_)
        | Op::ImpliedByCount(_)
        | Op::ParticipantTagGroupCount(_)
        | Op::ChannelPool(_)
        | Op::OutcomeSequence(_)
        | Op::InvokeOutcomeGraph(_)
        | Op::Arbitrate(_) => true,
    })
}

/// ADR-0143 D-1: shared allow-list for `turn_eligibility_pred` (MF-VAL-334).
///
/// This is deliberately a FORK of [`only_reads_actor_subject`], not a reuse
/// (canon-warden finding 8 on the design): that helper was audited against
/// the int grammar and admits "everything else" by default, which here would
/// let dice consume `ctx.rng` on every scanned candidate and let
/// `participant_tag_group_count`/`implied_by_count` read facts the
/// per-candidate context never fills (silently `0`/`false`).
///
/// The runtime context is `collect_entity_state_facts` for the candidate
/// only (int, effective-int, resource-bound, capability, tag and status
/// facts). Admitted, therefore:
/// - actor-subject `ReadInt`/`ReadEffectiveInt`/`ReadResourceMax`/`ReadResourceMin`;
/// - actor-subject `HasStatus`/`HasTag`/`HasCapability`;
/// - literals, comparison, boolean and arithmetic composition, `cond`, `let`.
///
/// Everything else is refused, including `HasNiche`/`HasItem` (the context
/// never populates niches or items, so they would be a silent-false trap),
/// every non-actor subject, dice and roll tables. An EXHAUSTIVE match with no
/// wildcard: a new `Op` forces a decision here.
///
/// Returns the first refused node's description on failure.
pub fn turn_eligibility_admits(expr: &spellkave_dsl::Expression) -> Result<(), String> {
    use spellkave_dsl::{Op, SubjectRef};
    for node in &expr.nodes {
        let refused: Option<&str> = match &node.op {
            Op::ReadInt(rf)
            | Op::ReadEffectiveInt(rf)
            | Op::ReadResourceMax(rf)
            | Op::ReadResourceMin(rf) => {
                (rf.subject != SubjectRef::Actor).then_some("a non-actor field read")
            }
            Op::HasCapability(h) => {
                (h.subject != SubjectRef::Actor).then_some("a non-actor has_capability")
            }
            Op::HasTag(h) => (h.subject != SubjectRef::Actor).then_some("a non-actor has_tag"),
            Op::HasStatus(h) => {
                (h.subject != SubjectRef::Actor).then_some("a non-actor status read")
            }
            Op::ReadBool(_) => Some("a bool state read (never populated for a candidate)"),
            Op::ReadText(_) => Some("a text read (never populated for a candidate)"),
            Op::HasNiche(_) => Some("has_niche (niches are never populated for a candidate)"),
            Op::HasItem(_) => Some("has_item (items are never populated for a candidate)"),
            Op::TracePresent(_) => Some("trace_present"),
            Op::KnowsEntity(_) => Some("knows_entity"),
            Op::SharesTag(_) => Some("shares_tag"),
            Op::PeersNearTarget(_) => Some("peers_near_target"),
            Op::OccupantsAt(_) => Some("occupants_at"),
            Op::DslSource(_) => Some("a data-defined source(...) slot"),
            Op::ImpliedByCount(_) => Some("implied_by_count"),
            Op::ParticipantTagGroupCount(_) => Some("participant_tag_group_count"),
            Op::ChannelPool(_) => Some("channel_pool"),
            Op::Roll(_) | Op::RollUnder(_) | Op::RollTable(_) => {
                Some("a dice roll or roll table (eligibility must be deterministic)")
            }
            Op::ModifyResource(_)
            | Op::ApplyStatus(_)
            | Op::EmitTrace(_)
            | Op::OutcomeSequence(_)
            | Op::InvokeOutcomeGraph(_)
            | Op::Arbitrate(_) => Some("an outcome operation"),
            Op::BoolLiteral(_)
            | Op::IntLiteral(_)
            | Op::TextLiteral(_)
            | Op::And(_)
            | Op::Or(_)
            | Op::Not(_)
            | Op::Compare(_)
            | Op::Arithmetic(_)
            | Op::Cond(_)
            | Op::Let(_) => None,
        };
        if let Some(what) = refused {
            return Err(what.to_owned());
        }
    }
    Ok(())
}

/// Parse `source` as a Bool predicate with the manifest macro closure and run
/// the shared [`turn_eligibility_admits`] allow-list. The runtime evaluator
/// calls the same two steps with the live world's macro closure.
pub fn compile_turn_eligibility(
    source: &str,
    macros: &[spellkave_dsl::DslMacroDefinition],
) -> Result<spellkave_dsl::Expression, String> {
    let expr = crate::dsl_macros::parse_predicate(source, macros).map_err(|failure| {
        failure
            .diagnostics
            .first()
            .map(|d| format!("parse error: {}", d.message))
            .unwrap_or_else(|| "parse error".to_owned())
    })?;
    turn_eligibility_admits(&expr)
        .map_err(|what| format!("reads {what}; only actor-subject facts are admitted"))?;
    Ok(expr)
}

/// ADR-0143 D-1 / MF-VAL-334: a nonblank `turn_eligibility_pred` compiles
/// through [`compile_turn_eligibility`]. The predicate lives on its own
/// `session_definitions` row, so its `session_def_id` resolves by
/// construction; there is no separate FK to check.
fn validate_session_turn_eligibility(
    manifest: &Manifest,
    macros: &[spellkave_dsl::DslMacroDefinition],
    diags: &mut Vec<Diagnostic>,
) {
    for row in &manifest.session_definitions {
        let source = row.turn_eligibility_pred.trim();
        if source.is_empty() {
            continue;
        }
        if let Err(reason) = compile_turn_eligibility(source, macros) {
            diags.push(Diagnostic {
                code: MF_VAL_SESSION_DEF_TURN_ELIGIBILITY.to_owned(),
                severity: Severity::Fatal,
                message: format!(
                    "session_definition '{}' turn_eligibility_pred '{}': {reason}",
                    row.session_def_id, row.turn_eligibility_pred
                ),
                path: format!(
                    "session_definitions[{}].turn_eligibility_pred",
                    row.session_def_id
                ),
                span: None,
                hint: Some(
                    "write a Bool predicate over the candidate as `actor` only, e.g. \
                     \"actor.status('<condition_id>') == false\" or a Bool macro"
                        .to_owned(),
                ),
                details: Some(serde_json::json!({ "session_def_id": row.session_def_id })),
            });
        }
    }
}

/// ADR-0037 D-H: validate `[[session_definitions]]` rows.
/// Checks `turn_scheduler_id` FK resolves when non-empty (within this manifest)
/// and combat-grid blueprints resolve through the bundle dependency closure.
fn validate_session_definitions(manifest: &Manifest, diags: &mut Vec<Diagnostic>) {
    let scheduler_ids: std::collections::HashSet<&str> = manifest
        .turn_scheduler
        .iter()
        .map(|s| s.id.as_str())
        .collect();
    let scope = BundleScope::build(manifest);
    let blueprint_bundle: BTreeMap<&str, &str> = manifest
        .combat_grid_blueprints
        .iter()
        .map(|row| (row.blueprint_id.as_str(), row.bundle_id.as_str()))
        .collect();
    let present: BTreeSet<&str> = manifest
        .bundles
        .iter()
        .map(|row| row.bundle_id.as_str())
        .collect();
    for row in &manifest.session_definitions {
        if !round_duration_is_whole_ticks(
            row.round_duration_ms,
            spellkave_schema::DEFAULT_FICTION_MS_PER_TICK,
        ) {
            diags.push(Diagnostic {
                code: MF_VAL_SESSION_DEF_ROUND_DURATION_QUANTIZATION.to_owned(),
                severity: Severity::Fatal,
                message: format!(
                    "session_definition '{}' round_duration_ms {} must be a positive multiple of fiction_ms_per_tick ({})",
                    row.session_def_id,
                    row.round_duration_ms,
                    spellkave_schema::DEFAULT_FICTION_MS_PER_TICK,
                ),
                path: format!("session_definitions[{}].round_duration_ms", row.session_def_id),
                span: None,
                hint: Some(
                    "use 0 for an unbound session, or a positive whole multiple of fiction_ms_per_tick"
                        .to_owned(),
                ),
                details: None,
            });
        }
        if !row.turn_scheduler_id.is_empty()
            && !scheduler_ids.contains(row.turn_scheduler_id.as_str())
        {
            diags.push(Diagnostic {
                code: MF_VAL_SESSION_DEF_SCHEDULER_FK.to_owned(),
                severity: Severity::Fatal,
                message: format!(
                    "session_definition '{}' turn_scheduler_id '{}' not found in this manifest's turn_scheduler rows",
                    row.session_def_id, row.turn_scheduler_id
                ),
                path: format!("session_definitions[{}].turn_scheduler_id", row.session_def_id),
                span: None,
                hint: Some("ensure the referenced turn_scheduler row is in the same or a dependency bundle".to_owned()),
                details: None,
            });
        }
        if !row.combat_grid_blueprint_id.is_empty() {
            match resolve_scoped_ref(
                row.bundle_id.as_str(),
                row.combat_grid_blueprint_id.as_str(),
                &blueprint_bundle,
                &scope,
                &present,
            ) {
                RefResolution::InScope | RefResolution::Trusted => {}
                RefResolution::OutOfScope { decl_bundle } => {
                    diags.push(Diagnostic {
                        code: MF_VAL_COMBAT_GRID_BLUEPRINT_REF.to_owned(),
                        severity: Severity::Fatal,
                        message: format!(
                            "session_definition '{}' combat-grid blueprint '{}' is declared by bundle '{}', which is not in '{}'s depends_on closure",
                            row.session_def_id,
                            row.combat_grid_blueprint_id,
                            decl_bundle,
                            row.bundle_id,
                        ),
                        path: format!(
                            "session_definitions[{}].combat_grid_blueprint_id",
                            row.session_def_id
                        ),
                        span: None,
                        hint: Some(format!(
                            "add '{decl_bundle}' to this bundle's depends_on, or use an in-scope combat-grid blueprint"
                        )),
                        details: None,
                    });
                }
                RefResolution::UndeclaredButVisible => {
                    diags.push(Diagnostic {
                        code: MF_VAL_COMBAT_GRID_BLUEPRINT_REF.to_owned(),
                        severity: Severity::Fatal,
                        message: format!(
                            "session_definition '{}' combat-grid blueprint '{}' not found in the visible dependency closure",
                            row.session_def_id, row.combat_grid_blueprint_id
                        ),
                        path: format!(
                            "session_definitions[{}].combat_grid_blueprint_id",
                            row.session_def_id
                        ),
                        span: None,
                        hint: Some(
                            "add an in-scope [[combat_grid_blueprints]] row or remove the session blueprint reference"
                                .to_owned(),
                        ),
                        details: None,
                    });
                }
            }
        }
    }
}

/// `0` preserves the established unbound-session sentinel. Any bound session
/// must advance fiction time by an exact positive number of ticks.
fn round_duration_is_whole_ticks(round_duration_ms: u64, fiction_ms_per_tick: u64) -> bool {
    round_duration_ms == 0
        || (fiction_ms_per_tick > 0 && round_duration_ms.is_multiple_of(fiction_ms_per_tick))
}

fn validate_combat_grid_blueprints(manifest: &Manifest, diags: &mut Vec<Diagnostic>) {
    let scope = BundleScope::build(manifest);
    let macros = crate::dsl_macros::dsl_macro_compiler_definitions(manifest);
    let template_bundle: BTreeMap<&str, &str> = manifest
        .affordance_templates
        .iter()
        .map(|row| (row.template_id.as_str(), row.bundle_id.as_str()))
        .collect();
    let present: BTreeSet<&str> = manifest
        .bundles
        .iter()
        .map(|b| b.bundle_id.as_str())
        .collect();
    // Authored metric vocabulary: metric id -> declaring bundle. The core seeds
    // ship with the module (`spatial::distance_metric::CORE_DISTANCE_METRICS`),
    // so they resolve even when no manifest re-declares them.
    let metric_bundle: BTreeMap<&str, &str> = spellkave_schema::CORE_DISTANCE_METRIC_IDS
        .iter()
        .map(|id| (*id, "core"))
        .chain(
            manifest
                .distance_metric_definitions
                .iter()
                .map(|row| (row.metric_id.as_str(), row.bundle_id.as_str())),
        )
        .collect();
    for row in &manifest.combat_grid_blueprints {
        if row.width == 0 || row.height == 0 {
            diags.push(Diagnostic {
                code: MF_VAL_COMBAT_GRID_BLUEPRINT_SHAPE.to_owned(),
                severity: Severity::Fatal,
                message: format!(
                    "combat_grid_blueprint '{}' must have positive width and height",
                    row.blueprint_id
                ),
                path: format!("combat_grid_blueprints[{}]", row.blueprint_id),
                span: None,
                hint: None,
                details: None,
            });
        }
        if !(row.cell_size_m.is_finite() && row.cell_size_m > 0.0) {
            diags.push(Diagnostic {
                code: MF_VAL_COMBAT_GRID_BLUEPRINT_CELL_SIZE.to_owned(),
                severity: Severity::Fatal,
                message: format!(
                    "combat_grid_blueprint '{}' cell_size_m must be finite and positive",
                    row.blueprint_id
                ),
                path: format!("combat_grid_blueprints[{}].cell_size_m", row.blueprint_id),
                span: None,
                hint: None,
                details: None,
            });
        }
        if !matches!(
            row.topology.as_str(),
            "4" | "4-neighbor" | "4-neighbour" | "8" | "8-neighbor" | "8-neighbour"
        ) {
            diags.push(Diagnostic {
                code: MF_VAL_COMBAT_GRID_BLUEPRINT_TOPOLOGY.to_owned(),
                severity: Severity::Fatal,
                message: format!(
                    "combat_grid_blueprint '{}' topology '{}' is not supported",
                    row.blueprint_id, row.topology
                ),
                path: format!("combat_grid_blueprints[{}].topology", row.blueprint_id),
                span: None,
                hint: Some("use '4' or '8'".to_owned()),
                details: None,
            });
        }
        validate_combat_grid_affordance_ref(
            row.bundle_id.as_str(),
            row.blueprint_id.as_str(),
            "edge_affordance_template_id",
            row.edge_affordance_template_id.as_str(),
            &template_bundle,
            &scope,
            &present,
            diags,
        );
        if let Some(metric_id) = row.distance_metric_id.as_deref() {
            validate_combat_grid_distance_metric_ref(
                row.bundle_id.as_str(),
                row.blueprint_id.as_str(),
                metric_id,
                &metric_bundle,
                &scope,
                &present,
                diags,
            );
        }
        validate_combat_grid_occupancy_read(
            manifest,
            &macros,
            row,
            &template_bundle,
            &scope,
            &present,
            diags,
        );
        if !matches!(
            serde_json::from_str::<serde_json::Value>(&row.edge_affordance_bindings_json)
                .map(|value| value.is_object()),
            Ok(true)
        ) {
            diags.push(Diagnostic {
                code: MF_VAL_COMBAT_GRID_BLUEPRINT_BINDINGS_JSON.to_owned(),
                severity: Severity::Fatal,
                message: format!(
                    "combat_grid_blueprint '{}' edge_affordance_bindings_json must be a JSON object",
                    row.blueprint_id
                ),
                path: format!(
                    "combat_grid_blueprints[{}].edge_affordance_bindings_json",
                    row.blueprint_id
                ),
                span: None,
                hint: None,
                details: None,
            });
        }
        if !matches!(
            serde_json::from_str::<serde_json::Value>(&row.edge_overrides_json)
                .map(|value| value.is_array()),
            Ok(true)
        ) {
            diags.push(Diagnostic {
                code: MF_VAL_COMBAT_GRID_BLUEPRINT_OVERRIDES_JSON.to_owned(),
                severity: Severity::Fatal,
                message: format!(
                    "combat_grid_blueprint '{}' edge_overrides_json must be a JSON array",
                    row.blueprint_id
                ),
                path: format!(
                    "combat_grid_blueprints[{}].edge_overrides_json",
                    row.blueprint_id
                ),
                span: None,
                hint: None,
                details: None,
            });
            continue;
        }
        let overrides =
            serde_json::from_str::<serde_json::Value>(&row.edge_overrides_json).unwrap_or_default();
        for (index, override_row) in overrides.as_array().into_iter().flatten().enumerate() {
            let Some(object) = override_row.as_object() else {
                diags.push(Diagnostic {
                    code: MF_VAL_COMBAT_GRID_BLUEPRINT_OVERRIDE_SHAPE.to_owned(),
                    severity: Severity::Fatal,
                    message: format!(
                        "combat_grid_blueprint '{}' edge override {} must be a JSON object",
                        row.blueprint_id, index
                    ),
                    path: format!(
                        "combat_grid_blueprints[{}].edge_overrides_json[{}]",
                        row.blueprint_id, index
                    ),
                    span: None,
                    hint: None,
                    details: None,
                });
                continue;
            };
            let from = object
                .get("from")
                .and_then(|value| value.as_str())
                .unwrap_or_default();
            let to = object
                .get("to")
                .and_then(|value| value.as_str())
                .unwrap_or_default();
            if from.is_empty() || to.is_empty() {
                diags.push(Diagnostic {
                    code: MF_VAL_COMBAT_GRID_BLUEPRINT_OVERRIDE_ENDPOINTS.to_owned(),
                    severity: Severity::Fatal,
                    message: format!(
                        "combat_grid_blueprint '{}' edge override {} must name non-empty from/to cells",
                        row.blueprint_id, index
                    ),
                    path: format!(
                        "combat_grid_blueprints[{}].edge_overrides_json[{}]",
                        row.blueprint_id, index
                    ),
                    span: None,
                    hint: Some("use local cell ids such as 'c_0_0' and 'c_1_0'".to_owned()),
                    details: None,
                });
            }
            if let Some(template_id) = object
                .get("affordance_template_id")
                .and_then(|value| value.as_str())
                .filter(|value| !value.is_empty())
            {
                validate_combat_grid_affordance_ref(
                    row.bundle_id.as_str(),
                    row.blueprint_id.as_str(),
                    &format!("edge_overrides_json[{index}].affordance_template_id"),
                    template_id,
                    &template_bundle,
                    &scope,
                    &present,
                    diags,
                );
            }
            if object
                .get("bindings")
                .is_some_and(|value| !value.is_object())
            {
                diags.push(Diagnostic {
                    code: MF_VAL_COMBAT_GRID_BLUEPRINT_OVERRIDE_BINDINGS.to_owned(),
                    severity: Severity::Fatal,
                    message: format!(
                        "combat_grid_blueprint '{}' edge override {} bindings must be a JSON object",
                        row.blueprint_id, index
                    ),
                    path: format!(
                        "combat_grid_blueprints[{}].edge_overrides_json[{}].bindings",
                        row.blueprint_id, index
                    ),
                    span: None,
                    hint: None,
                    details: None,
                });
            }
            if object.get("movement_cost_cells").is_some_and(|value| {
                !value
                    .as_f64()
                    .is_some_and(|cost| cost.is_finite() && cost > 0.0)
            }) {
                diags.push(Diagnostic {
                    code: MF_VAL_COMBAT_GRID_BLUEPRINT_OVERRIDE_COST.to_owned(),
                    severity: Severity::Fatal,
                    message: format!(
                        "combat_grid_blueprint '{}' edge override {} movement_cost_cells must be finite and positive",
                        row.blueprint_id, index
                    ),
                    path: format!(
                        "combat_grid_blueprints[{}].edge_overrides_json[{}].movement_cost_cells",
                        row.blueprint_id, index
                    ),
                    span: None,
                    hint: None,
                    details: None,
                });
            }
        }
    }
}

// Eight inputs is one over clippy's default; the params are all distinct
// validation context (refs, scope, present-set, diag sink), not a bag worth a
// struct. Newly flagged under rust-1.94's stricter default.
#[allow(clippy::too_many_arguments)]
fn validate_combat_grid_affordance_ref(
    using_bundle: &str,
    blueprint_id: &str,
    field_path: &str,
    template_id: &str,
    template_bundle: &BTreeMap<&str, &str>,
    scope: &BundleScope,
    present: &BTreeSet<&str>,
    diags: &mut Vec<Diagnostic>,
) {
    if template_id.is_empty() {
        combat_grid_affordance_ref_diag(blueprint_id, field_path, template_id, diags);
        return;
    }
    match resolve_scoped_ref(using_bundle, template_id, template_bundle, scope, present) {
        RefResolution::InScope | RefResolution::Trusted => {}
        RefResolution::OutOfScope { decl_bundle } => {
            diags.push(Diagnostic {
                code: MF_VAL_COMBAT_GRID_BLUEPRINT_AFFORDANCE_REF.to_owned(),
                severity: Severity::Fatal,
                message: format!(
                    "combat_grid_blueprint '{}' affordance template '{}' is declared by bundle '{}', which is not in '{}'s depends_on closure",
                    blueprint_id, template_id, decl_bundle, using_bundle
                ),
                path: format!("combat_grid_blueprints[{blueprint_id}].{field_path}"),
                span: None,
                hint: Some(format!(
                    "add '{decl_bundle}' to this bundle's depends_on, or use an in-scope traversal affordance"
                )),
                details: None,
            });
        }
        RefResolution::UndeclaredButVisible => {
            combat_grid_affordance_ref_diag(blueprint_id, field_path, template_id, diags);
        }
    }
}

/// ADR-0134 D-3: a blueprint whose edge template reads no `occupants_at`
/// must say `accepts_stacked_occupancy = true`. The read is detected on the
/// macro-expanded predicate AST (`Op::OccupantsAt`), never by substring; a
/// template whose predicate does not parse is skipped here — the parse
/// failure is its own diagnostic.
///
/// A template this manifest does not declare goes through the same two-tier
/// rule as every other cross-bundle ref (`resolve_scoped_ref`): with the
/// using bundle's `depends_on` closure fully present the template is a typo
/// or undeclared and the check fails closed; with the closure incomplete (a
/// per-bundle authoring run, e.g. the srd-cc proof scene referencing
/// `srd/affordance/step` from the base bundle) the ref is trusted and the
/// inspection is deferred to the pass that sees the whole closure (`just
/// seed` / live admission), exactly as MF-VAL-332 defers the metric id. An
/// out-of-scope declaring bundle is already reported by the affordance-ref
/// check and is not doubled here.
fn validate_combat_grid_occupancy_read(
    manifest: &Manifest,
    macros: &[spellkave_dsl::DslMacroDefinition],
    row: &spellkave_schema::CombatGridBlueprintRow,
    template_bundle: &BTreeMap<&str, &str>,
    scope: &BundleScope,
    present: &BTreeSet<&str>,
    diags: &mut Vec<Diagnostic>,
) {
    if row.accepts_stacked_occupancy {
        return;
    }
    let template = manifest
        .affordance_templates
        .iter()
        .find(|t| t.template_id == row.edge_affordance_template_id);
    if template.is_none() {
        match resolve_scoped_ref(
            row.bundle_id.as_str(),
            row.edge_affordance_template_id.as_str(),
            template_bundle,
            scope,
            present,
        ) {
            // Closure incomplete: the template lives in a dependency this
            // run cannot see. Trust it; the full-closure pass inspects it.
            RefResolution::Trusted => return,
            // Reported by `validate_combat_grid_affordance_ref` already.
            RefResolution::OutOfScope { .. } => return,
            // Declared here (impossible: `template` is None) or undeclared
            // with the whole closure visible: fail closed below.
            RefResolution::InScope | RefResolution::UndeclaredButVisible => {}
        }
    }
    let reads_occupants = match template {
        None => false,
        Some(t) if t.predicate_source.trim().is_empty() => false,
        Some(t) => match crate::dsl_macros::parse_predicate(&t.predicate_source, macros) {
            // Unparseable: reported by the template's own DSL diagnostic.
            Err(_) => return,
            Ok(expr) => expr
                .nodes
                .iter()
                .any(|node| matches!(node.op, spellkave_dsl::Op::OccupantsAt(_))),
        },
    };
    if reads_occupants {
        return;
    }
    let why = if template.is_some() {
        "reads no occupants_at(...) clause"
    } else {
        "is not declared in this manifest or its visible depends_on closure, so its predicate cannot be inspected"
    };
    diags.push(Diagnostic {
        code: MF_VAL_COMBAT_GRID_BLUEPRINT_STACKED_OCCUPANCY.to_owned(),
        severity: Severity::Fatal,
        message: format!(
            "combat_grid_blueprint '{}' edge template '{}' {why}: actors could stack on a cell",
            row.blueprint_id, row.edge_affordance_template_id
        ),
        path: format!(
            "combat_grid_blueprints[{}].edge_affordance_template_id",
            row.blueprint_id
        ),
        span: None,
        hint: Some(
            "gate the step on occupancy in the template predicate (e.g. `&& occupants_at('to_node', '<dead condition id>') == 0`), or set accepts_stacked_occupancy = true on the blueprint to state that stacking is intended"
                .to_owned(),
        ),
        details: None,
    });
}

/// ADR-0134 D-1: `combat_grid_blueprints[].distance_metric_id` resolves
/// against the visible `distance_metric_definitions` rows plus the core
/// seeds, under the same bundle-scope tiers as the affordance ref above.
fn validate_combat_grid_distance_metric_ref(
    using_bundle: &str,
    blueprint_id: &str,
    metric_id: &str,
    metric_bundle: &BTreeMap<&str, &str>,
    scope: &BundleScope,
    present: &BTreeSet<&str>,
    diags: &mut Vec<Diagnostic>,
) {
    let path = format!("combat_grid_blueprints[{blueprint_id}].distance_metric_id");
    let hint = Some(
        "use core/metric/euclidean, core/metric/chebyshev or core/metric/manhattan, or declare the id in [[distance_metric_definitions]]"
            .to_owned(),
    );
    if metric_id.is_empty() {
        diags.push(Diagnostic {
            code: MF_VAL_COMBAT_GRID_BLUEPRINT_DISTANCE_METRIC_REF.to_owned(),
            severity: Severity::Fatal,
            message: format!(
                "combat_grid_blueprint '{blueprint_id}' distance_metric_id must not be empty (omit the field for the Euclidean default)"
            ),
            path,
            span: None,
            hint,
            details: None,
        });
        return;
    }
    match resolve_scoped_ref(using_bundle, metric_id, metric_bundle, scope, present) {
        RefResolution::InScope | RefResolution::Trusted => {}
        RefResolution::OutOfScope { decl_bundle } => diags.push(Diagnostic {
            code: MF_VAL_COMBAT_GRID_BLUEPRINT_DISTANCE_METRIC_REF.to_owned(),
            severity: Severity::Fatal,
            message: format!(
                "combat_grid_blueprint '{blueprint_id}' distance metric '{metric_id}' is declared by bundle '{decl_bundle}', which is not in '{using_bundle}'s depends_on closure"
            ),
            path,
            span: None,
            hint: Some(format!(
                "add '{decl_bundle}' to this bundle's depends_on, or use a core/metric/* id"
            )),
            details: None,
        }),
        RefResolution::UndeclaredButVisible => diags.push(Diagnostic {
            code: MF_VAL_COMBAT_GRID_BLUEPRINT_DISTANCE_METRIC_REF.to_owned(),
            severity: Severity::Fatal,
            message: format!(
                "combat_grid_blueprint '{blueprint_id}' distance metric '{metric_id}' not found in this manifest's visible distance_metric_definitions rows"
            ),
            path,
            span: None,
            hint,
            details: None,
        }),
    }
}

fn combat_grid_affordance_ref_diag(
    blueprint_id: &str,
    field_path: &str,
    template_id: &str,
    diags: &mut Vec<Diagnostic>,
) {
    diags.push(Diagnostic {
        code: MF_VAL_COMBAT_GRID_BLUEPRINT_AFFORDANCE_REF.to_owned(),
        severity: Severity::Fatal,
        message: format!(
            "combat_grid_blueprint '{}' affordance template '{}' not found in this manifest's visible affordance_templates rows",
            blueprint_id, template_id
        ),
        path: format!("combat_grid_blueprints[{blueprint_id}].{field_path}"),
        span: None,
        hint: Some(
            "use a traversal affordance template such as core/affordance/traverse_open_portal"
                .to_owned(),
        ),
        details: None,
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use spellkave_dsl::parse_predicate;
    use spellkave_schema::{
        ActionDefinitionRow, ActionResolutionModeRow, AffordanceTemplateRow, AutonomyJobRow,
        BoundaryDefinitionRow, BundleRow, CombatGridBlueprintRow, ConsequenceFrameWriteRow,
        ConsequenceTemplateRow, EconomyChannelDefinitionRow, Manifest, SessionDefinitionRow,
        TurnSchedulerRow,
    };

    /// Minimal `action_definition` row carrying only `reaction_eligibility`
    /// under test; everything else is inert.
    fn action_def(action_id: &str, bundle: &str, reaction: Option<&str>) -> ActionDefinitionRow {
        ActionDefinitionRow {
            action_id: action_id.to_owned(),
            bundle_id: bundle.to_owned(),
            required_capability: String::new(),
            on_success: String::new(),
            on_failure: String::new(),
            resolution_mode: ActionResolutionModeRow::Deterministic,
            resolution_predicate_source: String::new(),
            resolution_scope: String::new(),
            non_steering: false,
            display_name: String::new(),
            cost_time_ms: 0,
            session_def_id: String::new(),
            resumable: false,
            reaction_eligibility: reaction.map(|s| s.to_owned()),
            reaction_window_ms: None,
            reaction_trigger: None,
            self_eligible: false,
            self_only: false,
            description: String::new(),
            spends_channel: None,
        }
    }

    /// As [`action_def`] but sets the §8.4 `reaction_trigger` (WHAT provokes)
    /// under test (slice F-impl, MF-VAL-239).
    fn action_def_trigger(action_id: &str, bundle: &str, trigger: &str) -> ActionDefinitionRow {
        ActionDefinitionRow {
            reaction_trigger: Some(trigger.to_owned()),
            ..action_def(action_id, bundle, Some(r#"actor.has_capability("cap")"#))
        }
    }

    /// Minimal `affordance_template` row carrying only the action-economy fields
    /// under test; everything else is inert.
    fn aff(
        template_id: &str,
        bundle: &str,
        consumes: Option<&str>,
        recharges: Option<&str>,
    ) -> AffordanceTemplateRow {
        AffordanceTemplateRow {
            template_id: template_id.to_owned(),
            bundle_id: bundle.to_owned(),
            niche_id: String::new(),
            capability_id: String::new(),
            display_name: String::new(),
            predicate_bool: false,
            predicate_source: String::new(),
            accepts_global_storm: false,
            locality_class: String::new(),
            reads_occupancy: false,
            description: String::new(),
            stratification_ok: true,
            outcome_template_id: String::new(),
            range_m: 0.0,
            spends_channel: consumes.map(|s| s.to_owned()),
            recharges_on: recharges.map(|s| s.to_owned()),
            composes: None,

            movement_cost_expression_source: String::new(),
        }
    }

    fn class(id: &str, bundle: &str, resets_on: &str) -> EconomyChannelDefinitionRow {
        EconomyChannelDefinitionRow {
            id: id.to_owned(),
            bundle_id: bundle.to_owned(),
            pool_ms_per_round: 1,
            resets_on: resets_on.to_owned(),
            off_turn_eligible: false,
            description: String::new(),
        }
    }

    fn boundary(id: &str, bundle: &str) -> BoundaryDefinitionRow {
        BoundaryDefinitionRow {
            id: id.to_owned(),
            bundle_id: bundle.to_owned(),
            description: String::new(),
        }
    }

    fn bundle(id: &str, deps: &[&str]) -> BundleRow {
        BundleRow {
            bundle_id: id.to_owned(),
            display_name: String::new(),
            author: String::new(),
            version: String::new(),
            depends_on: deps.iter().map(|s| (*s).to_owned()).collect(),
            description: String::new(),
            tag_prefixes: Vec::new(),
            allow_flat_tags: false,
            default_extent_shape_id: String::new(),
            default_extent_params_json: String::new(),
        }
    }

    fn boundary_job(job_id: &str, bundle_id: &str, boundary_id: &str) -> AutonomyJobRow {
        AutonomyJobRow {
            job_id: job_id.to_owned(),
            bundle_id: bundle_id.to_owned(),
            frequency_ms: 0,
            next_fire_at_ms: None,
            fire_at_boundary: boundary_id.to_owned(),
            owner_actor: 42,
            repeats_remaining: None,
            subject: None,
            subject_entity_key: String::new(),
            anchor_place_node_id: None,
            requires_subject_alive: false,
            dsl_function: "test/missing-effect".into(),
            payload_json: String::new(),
            priority: 0,
            enabled: true,
            source: "manifest".into(),
            description: String::new(),
            notable: false,
        }
    }

    fn scheduler(id: &str, kind: &str, active: bool) -> TurnSchedulerRow {
        TurnSchedulerRow {
            id: id.to_owned(),
            bundle_id: "core".to_owned(),
            scheduler_kind: kind.to_owned(),
            order_roll_source: None,
            active,
            description: String::new(),
        }
    }

    fn combat_grid_blueprint(
        blueprint_id: &str,
        bundle: &str,
        edge_template: &str,
    ) -> CombatGridBlueprintRow {
        CombatGridBlueprintRow {
            blueprint_id: blueprint_id.to_owned(),
            bundle_id: bundle.to_owned(),
            width: 2,
            height: 2,
            topology: "4".to_owned(),
            cell_size_m: 1.524, // 5 ft, converted x0.3048
            edge_affordance_template_id: edge_template.to_owned(),
            edge_affordance_bindings_json: "{}".to_owned(),
            edge_overrides_json: "[]".to_owned(),
            // The ref/metric tests below are not about occupancy; the
            // MF-VAL-333 tests flip this off explicitly.
            accepts_stacked_occupancy: true,
            description: String::new(),
            distance_metric_id: None,
        }
    }

    fn session_definition(
        session_def_id: &str,
        bundle: &str,
        combat_grid_blueprint_id: &str,
    ) -> SessionDefinitionRow {
        SessionDefinitionRow {
            session_def_id: session_def_id.to_owned(),
            bundle_id: bundle.to_owned(),
            participants_pred: String::new(),
            busy_allow_pred: None,
            busy_break_pred: None,
            initial_state: String::new(),
            legal_moves_csv: String::new(),
            turn_rule: String::new(),
            visibility_policy: String::new(),
            termination_pred: String::new(),
            on_terminate: String::new(),
            stakes_required: String::new(),
            winner_predicate: String::new(),
            turn_scheduler_id: String::new(),
            round_duration_ms: 0,
            auto_join_on_targeting: false,
            combat_grid_blueprint_id: combat_grid_blueprint_id.to_owned(),
            grid_edge_affordance_template_id: String::new(),
            grid_edge_affordance_bindings_json: String::new(),
            turn_eligibility_pred: String::new(),
            description: String::new(),
        }
    }

    fn codes(diags: &[Diagnostic]) -> Vec<&str> {
        diags.iter().map(|d| d.code.as_str()).collect()
    }

    #[test]
    fn clean_action_economy_passes() {
        // Core, self-contained: declares its boundary vocabulary and uses it.
        let m = Manifest {
            bundles: vec![bundle("core", &[])],
            boundary_definitions: vec![boundary("turn_start", "core"), boundary("never", "core")],
            economy_channel_definitions: vec![class(
                "core/action_class/primary",
                "core",
                "turn_start",
            )],
            turn_scheduler: vec![scheduler(
                "core/turn/round_robin_scheduler",
                "round_robin",
                true,
            )],
            ..Manifest::default()
        };
        let mut diags = Vec::new();
        validate_action_economy(&m, &mut diags);
        assert!(diags.is_empty(), "expected no diagnostics, got {diags:?}");
    }

    #[test]
    fn session_grid_blueprint_resolves_in_own_or_dependency_bundle() {
        let m = Manifest {
            bundles: vec![
                bundle("core", &[]),
                bundle("srd", &["core"]),
                bundle("adventure", &["srd"]),
            ],
            combat_grid_blueprints: vec![combat_grid_blueprint(
                "srd/combat-grid/standard-10x10",
                "srd",
                "srd/affordance/step",
            )],
            session_definitions: vec![
                session_definition(
                    "srd/session/combat",
                    "srd",
                    "srd/combat-grid/standard-10x10",
                ),
                session_definition(
                    "adventure/session/crypt-fight",
                    "adventure",
                    "srd/combat-grid/standard-10x10",
                ),
            ],
            ..Manifest::default()
        };

        let mut diags = Vec::new();
        validate_session_definitions(&m, &mut diags);
        assert!(
            diags.is_empty(),
            "expected dependency ref to resolve: {diags:?}"
        );
    }

    #[test]
    fn session_grid_blueprint_outside_dependency_scope_is_rejected() {
        let m = Manifest {
            bundles: vec![
                bundle("core", &[]),
                bundle("srd", &["core"]),
                bundle("adventure", &[]),
            ],
            combat_grid_blueprints: vec![combat_grid_blueprint(
                "srd/combat-grid/standard-10x10",
                "srd",
                "srd/affordance/step",
            )],
            session_definitions: vec![session_definition(
                "adventure/session/crypt-fight",
                "adventure",
                "srd/combat-grid/standard-10x10",
            )],
            ..Manifest::default()
        };

        let mut diags = Vec::new();
        validate_session_definitions(&m, &mut diags);
        assert_eq!(codes(&diags), vec![MF_VAL_COMBAT_GRID_BLUEPRINT_REF]);
    }

    #[test]
    fn session_grid_blueprint_is_trusted_while_dependency_rows_are_absent() {
        let m = Manifest {
            bundles: vec![bundle("adventure", &["srd"])],
            session_definitions: vec![session_definition(
                "adventure/session/crypt-fight",
                "adventure",
                "srd/combat-grid/standard-10x10",
            )],
            ..Manifest::default()
        };

        let mut diags = Vec::new();
        validate_session_definitions(&m, &mut diags);
        assert!(
            diags.is_empty(),
            "expected partial manifest to trust dependency ref: {diags:?}"
        );
    }

    #[test]
    fn missing_session_grid_blueprint_is_rejected_when_full_closure_is_visible() {
        let m = Manifest {
            bundles: vec![
                bundle("core", &[]),
                bundle("srd", &["core"]),
                bundle("adventure", &["srd"]),
            ],
            session_definitions: vec![session_definition(
                "adventure/session/crypt-fight",
                "adventure",
                "srd/combat-grid/standard-10x10",
            )],
            ..Manifest::default()
        };

        let mut diags = Vec::new();
        validate_session_definitions(&m, &mut diags);
        assert_eq!(codes(&diags), vec![MF_VAL_COMBAT_GRID_BLUEPRINT_REF]);
    }

    fn add_fire_boundary_source(manifest: &mut Manifest, bundle_id: &str, boundary_id: &str) {
        manifest.consequence_templates.push(ConsequenceTemplateRow {
            template_id: format!("{bundle_id}/rest/success"),
            bundle_id: bundle_id.to_owned(),
            effect_refs_csv: String::new(),
            traces_json: "[]".into(),
            description: String::new(),
        });
        manifest
            .consequence_frame_writes
            .push(ConsequenceFrameWriteRow {
                template_id: format!("{bundle_id}/rest/success"),
                ordinal: 0,
                frame_write: serde_json::json!({
                    "FireBoundary": { "boundary_id": boundary_id, "scope": "Actor" }
                }),
            });
    }

    #[test]
    fn non_kernel_boundary_reference_requires_reachable_fire_boundary() {
        let mut m = Manifest {
            bundles: vec![bundle("srd", &[])],
            boundary_definitions: vec![boundary("short_rest", "srd")],
            economy_channel_definitions: vec![class("srd/rest_pool", "srd", "short_rest")],
            ..Manifest::default()
        };

        let mut diags = Vec::new();
        validate_action_economy(&m, &mut diags);
        assert_eq!(codes(&diags), vec![MF_VAL_UNFIREABLE_BOUNDARY]);

        add_fire_boundary_source(&mut m, "srd", "short_rest");
        diags.clear();
        validate_action_economy(&m, &mut diags);
        assert!(
            diags.is_empty(),
            "FireBoundary must make the ref live: {diags:?}"
        );
    }

    #[test]
    fn recharge_boundary_uses_the_same_fireability_rule() {
        let mut m = Manifest {
            bundles: vec![bundle("srd", &[])],
            boundary_definitions: vec![boundary("long_rest", "srd")],
            affordance_templates: vec![aff("srd/breath", "srd", None, Some("long_rest"))],
            ..Manifest::default()
        };
        let mut diags = Vec::new();
        validate_action_economy(&m, &mut diags);
        assert_eq!(codes(&diags), vec![MF_VAL_UNFIREABLE_BOUNDARY]);

        add_fire_boundary_source(&mut m, "srd", "long_rest");
        diags.clear();
        validate_action_economy(&m, &mut diags);
        assert!(
            diags.is_empty(),
            "FireBoundary must make recharge live: {diags:?}"
        );
    }

    #[test]
    fn condition_expiry_boundary_uses_the_same_fireability_rule() {
        let mut m = Manifest {
            bundles: vec![bundle("srd", &[])],
            boundary_definitions: vec![boundary("long_rest", "srd")],
            consequence_templates: vec![ConsequenceTemplateRow {
                template_id: "srd/status/success".into(),
                bundle_id: "srd".into(),
                effect_refs_csv: String::new(),
                traces_json: "[]".into(),
                description: String::new(),
            }],
            consequence_frame_writes: vec![ConsequenceFrameWriteRow {
                template_id: "srd/status/success".into(),
                ordinal: 0,
                frame_write: serde_json::json!({
                    "ApplyStatus": {
                        "target_kind": "entity",
                        "target_ref": "TargetActor",
                        "condition_definition_id": "srd/rested",
                        "expires_at_boundary": {
                            "boundary_id": "long_rest",
                            "owner_actor": "TargetActor"
                        }
                    }
                }),
            }],
            ..Manifest::default()
        };

        let mut diags = Vec::new();
        validate_action_economy(&m, &mut diags);
        assert_eq!(codes(&diags), vec![MF_VAL_UNFIREABLE_BOUNDARY]);

        add_fire_boundary_source(&mut m, "srd", "long_rest");
        diags.clear();
        validate_action_economy(&m, &mut diags);
        assert!(
            diags.is_empty(),
            "FireBoundary must make condition expiry live: {diags:?}"
        );
    }

    #[test]
    fn autonomy_job_boundary_uses_the_same_ref_and_fireability_rules() {
        let mut m = Manifest {
            bundles: vec![bundle("core", &[]), bundle("srd", &[])],
            boundary_definitions: vec![boundary("short_rest", "srd")],
            autonomy_jobs: vec![boundary_job("srd/rest-job", "srd", "short_rest")],
            ..Manifest::default()
        };

        let mut diags = Vec::new();
        validate_action_economy(&m, &mut diags);
        assert_eq!(codes(&diags), vec![MF_VAL_UNFIREABLE_BOUNDARY]);

        add_fire_boundary_source(&mut m, "srd", "short_rest");
        diags.clear();
        validate_action_economy(&m, &mut diags);
        assert!(
            diags.is_empty(),
            "FireBoundary must make the autonomy arm live: {diags:?}"
        );

        m.autonomy_jobs[0].fire_at_boundary = "mistyped_rest".into();
        diags.clear();
        validate_action_economy(&m, &mut diags);
        assert_eq!(codes(&diags), vec![MF_VAL_AUTONOMY_JOB_BOUNDARY_REF]);
    }

    #[test]
    fn fire_boundary_source_must_reference_an_authored_boundary() {
        let mut m = Manifest {
            // The bundle scope includes the implicit core baseline, so include it
            // here to make the complete closure visible and exercise the typo arm.
            bundles: vec![bundle("core", &[]), bundle("srd", &[])],
            ..Manifest::default()
        };
        add_fire_boundary_source(&mut m, "srd", "mistyped_rest");
        let mut diags = Vec::new();
        validate_action_economy(&m, &mut diags);
        assert_eq!(codes(&diags), vec![MF_VAL_FIRE_BOUNDARY_REF]);
    }

    #[test]
    fn sub_tick_session_round_duration_is_rejected() {
        let m = Manifest {
            session_definitions: vec![spellkave_schema::SessionDefinitionRow {
                session_def_id: "test/sub_tick".to_owned(),
                bundle_id: "test".to_owned(),
                participants_pred: String::new(),
                busy_allow_pred: None,
                busy_break_pred: None,
                initial_state: String::new(),
                legal_moves_csv: String::new(),
                turn_rule: String::new(),
                visibility_policy: String::new(),
                termination_pred: String::new(),
                on_terminate: String::new(),
                stakes_required: String::new(),
                winner_predicate: String::new(),
                turn_scheduler_id: String::new(),
                round_duration_ms: 1_000,
                auto_join_on_targeting: false,
                combat_grid_blueprint_id: String::new(),
                grid_edge_affordance_template_id: String::new(),
                grid_edge_affordance_bindings_json: String::new(),
                turn_eligibility_pred: String::new(),
                description: String::new(),
            }],
            ..Manifest::default()
        };
        let mut diags = Vec::new();
        validate_action_economy(&m, &mut diags);
        assert_eq!(
            codes(&diags),
            vec![MF_VAL_SESSION_DEF_ROUND_DURATION_QUANTIZATION]
        );
    }

    fn turn_eligibility_diags(pred: &str) -> Vec<Diagnostic> {
        let mut m = Manifest {
            bundles: vec![bundle("srd", &[])],
            dsl_macro_definitions: vec![spellkave_schema::DslMacroDefinitionRow {
                macro_id: "srd/takes-turn".to_owned(),
                bundle_id: "srd".to_owned(),
                return_type: spellkave_dsl::ValueType::Bool,
                parameters: Vec::new(),
                body_source: "actor.status('srd/condition/dead') == false".to_owned(),
                description: String::new(),
            }],
            ..Manifest::default()
        };
        let mut row = session_definition("srd/session/combat", "srd", "");
        row.turn_eligibility_pred = pred.to_owned();
        m.session_definitions = vec![row];
        let macros = crate::dsl_macros::dsl_macro_compiler_definitions(&m);
        let mut diags = Vec::new();
        validate_session_turn_eligibility(&m, &macros, &mut diags);
        diags
    }

    /// ADR-0143 D-1: empty is today's unfiltered rotation, and the
    /// actor-subject shapes the design names are admitted, macros included.
    #[test]
    fn turn_eligibility_admits_actor_subject_predicates() {
        for pred in [
            "",
            "   ",
            "actor.status('srd/condition/dead') == false",
            "macro('srd/takes-turn')",
            "actor.stat('srd/hp') > 0 && actor.has_tag('srd/creature')",
            "actor.has_capability('core/act') || actor.effective('srd/hp') >= 1",
        ] {
            let diags = turn_eligibility_diags(pred);
            assert!(diags.is_empty(), "{pred:?} should pass: {diags:?}");
        }
    }

    /// ADR-0143 D-1 / canon-warden finding 8: each refused shape is Fatal
    /// MF-VAL-334 — non-Bool, target reads, dice, the int-grammar count ops,
    /// the never-populated pools, and an unknown macro.
    #[test]
    fn turn_eligibility_refuses_unbacked_shapes_as_fatal() {
        for pred in [
            "actor.stat('srd/hp')",
            "target.status('srd/condition/dead') == false",
            "d20 > 10",
            "participant_tag_group_count('srd/faction/') > 1",
            "actor.has_niche('srd/niche/x')",
            "actor.has_item('x')",
            "macro('srd/unknown')",
            "actor.status(",
        ] {
            let diags = turn_eligibility_diags(pred);
            assert_eq!(
                codes(&diags),
                vec![MF_VAL_SESSION_DEF_TURN_ELIGIBILITY],
                "{pred:?} should be refused"
            );
            assert_eq!(diags[0].severity, Severity::Fatal);
        }
        // The count ops, dice and pool reads parse fine; the allow-list is
        // what refuses them, not the parser.
        for pred in [
            "d20 > 10",
            "participant_tag_group_count('srd/faction/') > 1",
            "actor.has_niche('srd/niche/x')",
            "actor.has_item('x')",
            "target.status('srd/condition/dead') == false",
        ] {
            let diags = turn_eligibility_diags(pred);
            assert!(
                diags[0].message.contains("only actor-subject facts"),
                "{pred:?} must be refused by the allow-list: {}",
                diags[0].message
            );
        }
    }

    #[test]
    fn quantization_accepts_exact_configured_tick_multiples() {
        assert!(round_duration_is_whole_ticks(12_000, 6_000));
        assert!(!round_duration_is_whole_ticks(6_000, 60_000));
        assert!(round_duration_is_whole_ticks(0, 60_000));
    }

    #[test]
    fn unknown_resets_on_rejected_when_closure_visible() {
        // Self-contained core, full vocabulary visible — a typo / undeclared
        // boundary is a hard error (existence tier).
        let m = Manifest {
            bundles: vec![bundle("core", &[])],
            boundary_definitions: vec![boundary("turn_start", "core")],
            economy_channel_definitions: vec![class("core/action_class/x", "core", "lunar_cycle")],
            ..Manifest::default()
        };
        let mut diags = Vec::new();
        validate_action_economy(&m, &mut diags);
        assert_eq!(codes(&diags), vec![MF_VAL_CHANNEL_RESETS_ON]);
    }

    #[test]
    fn cross_bundle_boundary_trusted_when_dependency_absent() {
        // Regression guard: validating `srd` ALONE (core's boundary rows
        // absent), an srd class resetting on core's `turn_start` MUST NOT
        // false-reject — the dependency's vocabulary is simply not in this
        // per-file manifest. core ∉ present → existence tier stays lenient.
        let m = Manifest {
            bundles: vec![bundle("srd", &["core"])],
            boundary_definitions: vec![boundary("short_rest", "srd")],
            economy_channel_definitions: vec![class("srd/action_class/bonus", "srd", "turn_start")],
            ..Manifest::default()
        };
        let mut diags = Vec::new();
        validate_action_economy(&m, &mut diags);
        assert!(
            diags.is_empty(),
            "cross-bundle boundary ref must be trusted per-file, got {diags:?}"
        );
    }

    #[test]
    fn combat_grid_core_affordance_trusted_when_dependency_absent() {
        let m = Manifest {
            bundles: vec![bundle("srd", &["core"])],
            combat_grid_blueprints: vec![combat_grid_blueprint(
                "srd/combat-grid/default",
                "srd",
                "core/affordance/traverse_open_portal",
            )],
            ..Manifest::default()
        };
        let mut diags = Vec::new();
        validate_action_economy(&m, &mut diags);
        assert!(
            diags.is_empty(),
            "cross-bundle combat grid affordance must be trusted per-file, got {diags:?}"
        );
    }

    /// ADR-0134 D-1: the three core metric ids resolve without any
    /// `[[distance_metric_definitions]]` row in the manifest.
    #[test]
    fn combat_grid_core_distance_metric_resolves_without_declaration() {
        let mut m = Manifest {
            bundles: vec![bundle("core", &[])],
            affordance_templates: vec![aff(
                "core/affordance/traverse_open_portal",
                "core",
                None,
                None,
            )],
            ..Manifest::default()
        };
        let mut row = combat_grid_blueprint(
            "core/combat-grid/default",
            "core",
            "core/affordance/traverse_open_portal",
        );
        row.distance_metric_id = Some("core/metric/chebyshev".to_owned());
        m.combat_grid_blueprints = vec![row];
        let mut diags = Vec::new();
        validate_action_economy(&m, &mut diags);
        assert!(diags.is_empty(), "core metric must resolve, got {diags:?}");
    }

    /// ADR-0134 D-3: a blueprint whose edge template reads no `occupants_at`
    /// lets actors stack — refused (MF-VAL-333) unless the blueprint says so.
    #[test]
    fn combat_grid_without_an_occupancy_read_is_refused_unless_annotated() {
        let mut m = Manifest {
            bundles: vec![bundle("core", &[])],
            affordance_templates: vec![aff(
                "core/affordance/traverse_open_portal",
                "core",
                None,
                None,
            )],
            ..Manifest::default()
        };
        let mut row = combat_grid_blueprint(
            "core/combat-grid/default",
            "core",
            "core/affordance/traverse_open_portal",
        );
        row.accepts_stacked_occupancy = false;
        m.combat_grid_blueprints = vec![row.clone()];
        let mut diags = Vec::new();
        validate_action_economy(&m, &mut diags);
        assert_eq!(
            codes(&diags),
            vec![MF_VAL_COMBAT_GRID_BLUEPRINT_STACKED_OCCUPANCY]
        );
        assert_eq!(diags[0].severity, Severity::Fatal);
        assert_eq!(
            diags[0].path,
            "combat_grid_blueprints[core/combat-grid/default].edge_affordance_template_id"
        );

        row.accepts_stacked_occupancy = true;
        m.combat_grid_blueprints = vec![row];
        let mut diags = Vec::new();
        validate_action_economy(&m, &mut diags);
        assert!(
            diags.is_empty(),
            "annotated blueprint passes, got {diags:?}"
        );
    }

    /// ADR-0134 D-3: the SRD clause on the edge template satisfies the
    /// control; the read is found on the expanded AST, so a macro that
    /// expands to it counts too.
    #[test]
    fn combat_grid_with_an_occupancy_read_on_its_edge_template_passes() {
        let mut m = Manifest {
            bundles: vec![bundle("srd", &[])],
            ..Manifest::default()
        };
        let mut step = aff("srd/affordance/step", "srd", None, None);
        step.predicate_source =
            "actor.has_capability('srd/capability/walk') && occupants_at('to_node', 'srd/condition/dead') == 0"
                .to_owned();
        let mut with_macro = aff("srd/affordance/step_macro", "srd", None, None);
        with_macro.predicate_source = "macro('srd/macro/cell_free')".to_owned();
        m.affordance_templates = vec![step, with_macro];
        m.dsl_macro_definitions = vec![spellkave_schema::DslMacroDefinitionRow {
            macro_id: "srd/macro/cell_free".to_owned(),
            bundle_id: "srd".to_owned(),
            return_type: spellkave_schema::DslValueType::Bool,
            parameters: Vec::new(),
            body_source: "occupants_at('to_node') == 0".to_owned(),
            description: String::new(),
        }];
        for template in ["srd/affordance/step", "srd/affordance/step_macro"] {
            let mut row = combat_grid_blueprint("srd/combat-grid/standard", "srd", template);
            row.accepts_stacked_occupancy = false;
            m.combat_grid_blueprints = vec![row];
            let mut diags = Vec::new();
            validate_action_economy(&m, &mut diags);
            assert!(
                diags.is_empty(),
                "template {template} reads occupants_at, got {diags:?}"
            );
        }
    }

    /// ADR-0134 D-3 + the two-tier ref rule: a template declared by a
    /// dependency this run cannot see (closure incomplete — the per-bundle
    /// authoring pass, e.g. the srd-cc proof scene binding the base bundle's
    /// `srd/affordance/step`) is trusted, the way MF-VAL-332 trusts a metric
    /// id. The full-closure pass inspects the real predicate.
    #[test]
    fn combat_grid_with_an_edge_template_in_an_unseen_dependency_is_trusted() {
        let mut m = Manifest {
            bundles: vec![bundle("srd_proof", &["srd"])],
            ..Manifest::default()
        };
        let mut row = combat_grid_blueprint(
            "srd_proof/combat-grid/arena",
            "srd_proof",
            "srd/affordance/step",
        );
        row.accepts_stacked_occupancy = false;
        m.combat_grid_blueprints = vec![row];
        let mut diags = Vec::new();
        validate_action_economy(&m, &mut diags);
        assert!(
            diags.is_empty(),
            "closure-incomplete ref must be trusted, got {diags:?}"
        );
    }

    /// ADR-0134 D-3: with the whole `depends_on` closure present, a template
    /// nobody declares cannot be inspected — fail closed next to the
    /// affordance-ref diagnostic; the annotation is the only way through.
    #[test]
    fn combat_grid_with_an_undeclared_edge_template_fails_closed_when_the_closure_is_visible() {
        let mut m = Manifest {
            bundles: vec![bundle("core", &[]), bundle("mod", &["core"])],
            ..Manifest::default()
        };
        let mut row = combat_grid_blueprint("mod/combat-grid", "mod", "core/affordance/missing");
        row.accepts_stacked_occupancy = false;
        m.combat_grid_blueprints = vec![row.clone()];
        let mut diags = Vec::new();
        validate_action_economy(&m, &mut diags);
        assert_eq!(
            codes(&diags),
            vec![
                MF_VAL_COMBAT_GRID_BLUEPRINT_AFFORDANCE_REF,
                MF_VAL_COMBAT_GRID_BLUEPRINT_STACKED_OCCUPANCY
            ]
        );
        assert!(
            diags[1].message.contains("not declared in this manifest"),
            "{}",
            diags[1].message
        );

        row.accepts_stacked_occupancy = true;
        m.combat_grid_blueprints = vec![row];
        let mut diags = Vec::new();
        validate_action_economy(&m, &mut diags);
        assert_eq!(
            codes(&diags),
            vec![MF_VAL_COMBAT_GRID_BLUEPRINT_AFFORDANCE_REF],
            "the annotation silences only MF-VAL-333"
        );
    }

    /// ADR-0134 D-1: an unknown metric id is an authoring error (MF-VAL-332),
    /// never a silent Euclidean fallthrough.
    #[test]
    fn combat_grid_unknown_distance_metric_rejected_when_closure_visible() {
        let mut m = Manifest {
            bundles: vec![bundle("core", &[])],
            affordance_templates: vec![aff(
                "core/affordance/traverse_open_portal",
                "core",
                None,
                None,
            )],
            ..Manifest::default()
        };
        let mut row = combat_grid_blueprint(
            "core/combat-grid/default",
            "core",
            "core/affordance/traverse_open_portal",
        );
        row.distance_metric_id = Some("core/metric/hexagonal".to_owned());
        m.combat_grid_blueprints = vec![row];
        let mut diags = Vec::new();
        validate_action_economy(&m, &mut diags);
        assert_eq!(
            codes(&diags),
            vec![MF_VAL_COMBAT_GRID_BLUEPRINT_DISTANCE_METRIC_REF]
        );
        assert_eq!(
            diags[0].path,
            "combat_grid_blueprints[core/combat-grid/default].distance_metric_id"
        );
    }

    /// A modder metric declared in-scope resolves; the same id from a bundle
    /// outside the closure is out of scope.
    #[test]
    fn combat_grid_declared_distance_metric_honours_bundle_scope() {
        let metric = |bundle: &str| spellkave_schema::DistanceMetricDefinitionRow {
            metric_id: "homebrew/metric/hex".to_owned(),
            metric_shape: "Chebyshev".to_owned(),
            bundle_id: bundle.to_owned(),
            description: String::new(),
        };
        let mut m = Manifest {
            bundles: vec![bundle("core", &[]), bundle("homebrew", &["core"])],
            affordance_templates: vec![aff(
                "core/affordance/traverse_open_portal",
                "core",
                None,
                None,
            )],
            distance_metric_definitions: vec![metric("homebrew")],
            ..Manifest::default()
        };
        let mut row = combat_grid_blueprint(
            "homebrew/combat-grid/hex",
            "homebrew",
            "core/affordance/traverse_open_portal",
        );
        row.distance_metric_id = Some("homebrew/metric/hex".to_owned());
        m.combat_grid_blueprints = vec![row.clone()];
        let mut diags = Vec::new();
        validate_action_economy(&m, &mut diags);
        assert!(diags.is_empty(), "in-scope metric resolves, got {diags:?}");

        // `core` does not depend on `homebrew`: out of scope.
        row.blueprint_id = "core/combat-grid/hex".to_owned();
        row.bundle_id = "core".to_owned();
        m.combat_grid_blueprints = vec![row];
        let mut diags = Vec::new();
        validate_action_economy(&m, &mut diags);
        assert_eq!(
            codes(&diags),
            vec![MF_VAL_COMBAT_GRID_BLUEPRINT_DISTANCE_METRIC_REF]
        );
    }

    #[test]
    fn combat_grid_unknown_affordance_rejected_when_closure_visible() {
        let m = Manifest {
            bundles: vec![bundle("core", &[])],
            combat_grid_blueprints: vec![combat_grid_blueprint(
                "core/combat-grid/default",
                "core",
                "core/affordance/missing",
            )],
            ..Manifest::default()
        };
        let mut diags = Vec::new();
        validate_action_economy(&m, &mut diags);
        assert_eq!(
            codes(&diags),
            vec![MF_VAL_COMBAT_GRID_BLUEPRINT_AFFORDANCE_REF]
        );
    }

    #[test]
    fn out_of_scope_boundary_rejected() {
        // A boundary IS declared in this manifest but by a bundle the using row
        // does not depend on (scope tier). `core` does not depend on `mod`.
        let m = Manifest {
            bundles: vec![bundle("core", &[]), bundle("mod", &[])],
            boundary_definitions: vec![boundary("encounter_end", "mod")],
            economy_channel_definitions: vec![class(
                "core/action_class/primary",
                "core",
                "encounter_end",
            )],
            ..Manifest::default()
        };
        let mut diags = Vec::new();
        validate_action_economy(&m, &mut diags);
        assert_eq!(codes(&diags), vec![MF_VAL_CHANNEL_RESETS_ON]);
    }

    #[test]
    fn bare_file_without_bundles_trusts_all() {
        // No `[[bundles]]` topology (a raw rows file): the existence tier never
        // fires, so resolution is fully lenient — never a false reject.
        let m = Manifest {
            economy_channel_definitions: vec![class(
                "core/action_class/primary",
                "core",
                "turn_start",
            )],
            ..Manifest::default()
        };
        let mut diags = Vec::new();
        validate_action_economy(&m, &mut diags);
        assert!(diags.is_empty(), "bare file must be lenient, got {diags:?}");
    }

    #[test]
    fn off_turn_eligible_zero_capacity_warns() {
        // #1241 soft guard: an off-turn-eligible class with capacity 0 is a dead
        // class (always exhausted) → Warning, not Fatal.
        let mut m = Manifest {
            bundles: vec![bundle("srd", &["core"])],
            boundary_definitions: vec![boundary("turn_start", "srd")],
            ..Manifest::default()
        };
        let mut dead = class("srd/action_class/reaction", "srd", "turn_start");
        dead.off_turn_eligible = true;
        dead.pool_ms_per_round = 0;
        m.economy_channel_definitions = vec![dead];
        let mut diags = Vec::new();
        validate_action_economy(&m, &mut diags);
        assert_eq!(codes(&diags), vec![MF_VAL_OFF_TURN_ELIGIBLE_ZERO_POOL]);
        assert!(diags.iter().all(|d| d.severity == Severity::Warning));
    }

    #[test]
    fn off_turn_eligible_with_capacity_is_clean() {
        // The shipped reaction class: off_turn_eligible + capacity 1 → no diag.
        let mut m = Manifest {
            bundles: vec![bundle("srd", &["core"])],
            boundary_definitions: vec![boundary("turn_start", "srd")],
            ..Manifest::default()
        };
        let mut reaction = class("srd/action_class/reaction", "srd", "turn_start");
        reaction.off_turn_eligible = true;
        m.economy_channel_definitions = vec![reaction];
        let mut diags = Vec::new();
        validate_action_economy(&m, &mut diags);
        assert!(
            diags.is_empty(),
            "off-turn-eligible class with capacity ≥ 1 must be clean, got {diags:?}"
        );
    }

    #[test]
    fn duplicate_channel_id_rejected() {
        let m = Manifest {
            bundles: vec![bundle("core", &[])],
            boundary_definitions: vec![boundary("turn_start", "core")],
            economy_channel_definitions: vec![
                class("core/action_class/primary", "core", "turn_start"),
                class("core/action_class/primary", "core", "turn_start"),
            ],
            ..Manifest::default()
        };
        let mut diags = Vec::new();
        validate_action_economy(&m, &mut diags);
        assert!(codes(&diags).contains(&MF_VAL_CHANNEL_DUP));
    }

    #[test]
    fn unknown_scheduler_kind_rejected() {
        let m = Manifest {
            turn_scheduler: vec![scheduler("mod/turn/x", "initiative", true)],
            ..Manifest::default()
        };
        let mut diags = Vec::new();
        validate_action_economy(&m, &mut diags);
        assert_eq!(codes(&diags), vec![MF_VAL_TURN_SCHEDULER_KIND]);
    }

    #[test]
    fn multiple_active_schedulers_rejected() {
        let m = Manifest {
            turn_scheduler: vec![
                scheduler("core/turn/round_robin_scheduler", "round_robin", true),
                scheduler("mod/turn/other", "round_robin", true),
            ],
            ..Manifest::default()
        };
        let mut diags = Vec::new();
        validate_action_economy(&m, &mut diags);
        assert_eq!(codes(&diags), vec![MF_VAL_TURN_SCHEDULER_ACTIVE]);
    }

    /// MF-VAL-240 routes the `order_roll_source` check through the SHARED DSL
    /// (`parse_int_expression` + eval against empty facts) — the same path the
    /// runtime takes (CR-17 / LR-17). It accepts any self-contained dice/int
    /// expression and rejects empty / unparseable / typo'd-binding sources.
    #[test]
    fn rolled_order_order_roll_source_routes_through_shared_dsl() {
        let rolled = |src: Option<&str>| TurnSchedulerRow {
            id: "srd/turn/initiative".to_owned(),
            bundle_id: "core".to_owned(),
            scheduler_kind: "rolled_order".to_owned(),
            order_roll_source: src.map(str::to_owned),
            active: true,
            description: String::new(),
        };
        let accepts = |src: Option<&str>| {
            let m = Manifest {
                bundles: vec![bundle("core", &[])],
                turn_scheduler: vec![rolled(src)],
                ..Manifest::default()
            };
            let mut diags = Vec::new();
            validate_action_economy(&m, &mut diags);
            !codes(&diags).contains(&MF_VAL_ROLLED_ORDER_SOURCE)
        };
        // Accepted: bare die, multiplier (count now honored at eval), arithmetic.
        assert!(accepts(Some("d20")), "d20 is a valid roll");
        assert!(accepts(Some("2d6")), "2d6 is a valid roll");
        assert!(accepts(Some("d20 - 2")), "dice arithmetic is a valid roll");
        // Rejected: empty (non-empty rule); unparseable; and a `d2O` typo (letter
        // O) that parses as a binding but cannot evaluate against empty facts —
        // exactly the author-time guard the deleted bespoke parser gave.
        assert!(!accepts(None), "missing source rejected");
        assert!(!accepts(Some("")), "empty source rejected");
        assert!(!accepts(Some("garbage")), "non-roll binding rejected");
        assert!(!accepts(Some("d2O")), "d2O typo rejected");
        assert!(!accepts(Some("2d")), "malformed dice rejected");
    }

    /// Finding #26: an Actor-subject field read is now accepted — the field
    /// is populated per-participant at RUNTIME
    /// (`r_session_open::resolve_initiative`), which the validator cannot
    /// know ahead of time, so an `UnknownField{subject: Actor}` evaluation
    /// error is tolerated. Target/`ActorWieldedItem`-scoped reads stay
    /// rejected — initiative has no target, so they would be meaningless.
    #[test]
    fn order_roll_source_actor_subject_reads_accepted_others_rejected() {
        let rolled = |src: &str| TurnSchedulerRow {
            id: "srd/turn/initiative".to_owned(),
            bundle_id: "core".to_owned(),
            scheduler_kind: "rolled_order".to_owned(),
            order_roll_source: Some(src.to_owned()),
            active: true,
            description: String::new(),
        };
        let accepts = |src: &str| {
            let m = Manifest {
                bundles: vec![bundle("core", &[])],
                turn_scheduler: vec![rolled(src)],
                ..Manifest::default()
            };
            let mut diags = Vec::new();
            validate_action_economy(&m, &mut diags);
            !codes(&diags).contains(&MF_VAL_ROLLED_ORDER_SOURCE)
        };
        // A bare Actor-subject stat read — the field is unknown to the
        // validator (empty facts), tolerated as the accepted runtime-resolved
        // shape.
        assert!(
            accepts(r#"actor.stat("srd/dex")"#),
            "bare actor.stat read accepted"
        );
        // The real `initiativeRollExpr` idiom (order-of-combat-units.ts):
        // `d20 + <Dex modifier>`, the modifier itself
        // `abilityModifierExpr`'s floor-division-with-correction shape.
        assert!(
            accepts(
                r#"d20 + (((actor.stat("srd/dex")) - 10) / 2 + cond(((actor.stat("srd/dex")) - 10) - (((actor.stat("srd/dex")) - 10) / 2) * 2 < 0, -1, 0))"#
            ),
            "d20 + Dex-modifier idiom accepted"
        );
        // Target-subject reads are meaningless — initiative has no target —
        // and stay rejected.
        assert!(
            !accepts(r#"target.stat("srd/dex")"#),
            "target-subject read rejected"
        );
        // ActorWieldedItem-subject reads are equally meaningless here.
        assert!(
            !accepts(r#"actor.wielded_item.stat("srd/reach")"#),
            "wielded-item-subject read rejected"
        );
        // Canon-warden follow-up (2026-07-28): Actor-subject Bool/Text reads
        // are REJECTED too, even though the subject is Actor — this call
        // site's runtime context never populates `bool_fields` at all, and
        // Actor-subject `text_fields` structurally never exist (only
        // `ActorWieldedItem` gets text facts). Accepting either would
        // hard-abort `r_session_open` for EVERY participant, unlike the
        // accepted ReadInt risk (a real, sometimes-missing per-participant
        // stat).
        assert!(
            !accepts(r#"cond(actor.state("srd/some_flag"), 1, 0)"#),
            "actor.state(...) (ReadBool) rejected"
        );
        assert!(
            !accepts(r#"roll_table(actor.substrate_id)"#),
            "actor.substrate_id (ReadText) rejected"
        );
    }

    /// #2218: `order_roll_source` is already parsed with the manifest macro
    /// closure. An unknown call must fail offline (`MF-VAL-240`) so a live
    /// `r_session_open` never sees `MF-DSL-MACRO-UNKNOWN` for a fixture the
    /// merge-gate already admitted.
    #[test]
    fn order_roll_source_unknown_macro_is_rejected() {
        let m = Manifest {
            bundles: vec![bundle("core", &[])],
            turn_scheduler: vec![TurnSchedulerRow {
                id: "srd/turn/initiative".to_owned(),
                bundle_id: "core".to_owned(),
                scheduler_kind: "rolled_order".to_owned(),
                order_roll_source: Some("d20 + (macro('srd/dexterity_modifier'))".to_owned()),
                active: true,
                description: String::new(),
            }],
            ..Manifest::default()
        };
        let mut diags = Vec::new();
        validate_action_economy(&m, &mut diags);
        assert!(
            codes(&diags).contains(&MF_VAL_ROLLED_ORDER_SOURCE),
            "unknown initiative macro must fail MF-VAL-240; got {diags:?}"
        );
    }

    #[test]
    fn order_roll_source_known_macro_is_admitted() {
        let mut m = Manifest {
            bundles: vec![bundle("srd", &["core"]), bundle("core", &[])],
            ..Manifest::default()
        };
        m.dsl_macro_definitions
            .push(spellkave_schema::DslMacroDefinitionRow {
                macro_id: "srd/dexterity_modifier".into(),
                bundle_id: "srd".into(),
                return_type: spellkave_schema::DslValueType::Int,
                parameters: Vec::new(),
                body_source: "((actor.stat('srd/dex') - 10) / 2)".into(),
                description: String::new(),
            });
        m.turn_scheduler = vec![TurnSchedulerRow {
            id: "srd/turn/initiative".to_owned(),
            bundle_id: "srd".to_owned(),
            scheduler_kind: "rolled_order".to_owned(),
            order_roll_source: Some("d20 + (macro('srd/dexterity_modifier'))".to_owned()),
            active: true,
            description: String::new(),
        }];
        let mut diags = Vec::new();
        validate_action_economy(&m, &mut diags);
        assert!(
            !codes(&diags).contains(&MF_VAL_ROLLED_ORDER_SOURCE),
            "declared initiative macro must parse; got {diags:?}"
        );
    }

    #[test]
    fn spends_channel_resolves_to_in_scope_class() {
        // Core declares the class; a core affordance consumes it — clean.
        let m = Manifest {
            bundles: vec![bundle("core", &[])],
            boundary_definitions: vec![boundary("turn_start", "core")],
            economy_channel_definitions: vec![class(
                "core/action_class/primary",
                "core",
                "turn_start",
            )],
            affordance_templates: vec![aff(
                "core/swing",
                "core",
                Some("core/action_class/primary"),
                None,
            )],
            ..Manifest::default()
        };
        let mut diags = Vec::new();
        validate_action_economy(&m, &mut diags);
        assert!(diags.is_empty(), "expected clean, got {diags:?}");
    }

    #[test]
    fn unknown_spends_channel_rejected_when_closure_visible() {
        // Full closure visible (core alone) + a typo'd class id → FK reject.
        let m = Manifest {
            bundles: vec![bundle("core", &[])],
            boundary_definitions: vec![boundary("turn_start", "core")],
            economy_channel_definitions: vec![class(
                "core/action_class/primary",
                "core",
                "turn_start",
            )],
            affordance_templates: vec![aff(
                "core/swing",
                "core",
                Some("core/action_class/typo"),
                None,
            )],
            ..Manifest::default()
        };
        let mut diags = Vec::new();
        validate_action_economy(&m, &mut diags);
        assert_eq!(codes(&diags), vec![MF_VAL_SPENDS_CHANNEL]);
    }

    #[test]
    fn spends_channel_trusted_when_dependency_absent() {
        // srd validated ALONE (core's class rows absent): an srd affordance
        // consuming core's class must NOT false-reject — same per-file leniency
        // resets_on uses.
        let m = Manifest {
            bundles: vec![bundle("srd", &["core"])],
            affordance_templates: vec![aff(
                "srd/breath",
                "srd",
                Some("core/action_class/primary"),
                None,
            )],
            ..Manifest::default()
        };
        let mut diags = Vec::new();
        validate_action_economy(&m, &mut diags);
        assert!(
            diags.is_empty(),
            "cross-bundle FK trusted per-file, got {diags:?}"
        );
    }

    #[test]
    fn recharges_on_resolves_to_authored_boundary() {
        // Recharge boundary is authored vocabulary (warden condition 7) — the
        // same boundary_definitions resets_on uses.
        let m = Manifest {
            bundles: vec![bundle("core", &[])],
            boundary_definitions: vec![boundary("turn_start", "core")],
            affordance_templates: vec![aff("core/breath", "core", None, Some("turn_start"))],
            ..Manifest::default()
        };
        let mut diags = Vec::new();
        validate_action_economy(&m, &mut diags);
        assert!(diags.is_empty(), "expected clean, got {diags:?}");
    }

    #[test]
    fn unknown_recharges_on_rejected_when_closure_visible() {
        let m = Manifest {
            bundles: vec![bundle("core", &[])],
            boundary_definitions: vec![boundary("turn_start", "core")],
            affordance_templates: vec![aff("core/breath", "core", None, Some("eclipse"))],
            ..Manifest::default()
        };
        let mut diags = Vec::new();
        validate_action_economy(&m, &mut diags);
        assert_eq!(codes(&diags), vec![MF_VAL_RECHARGES_ON]);
    }

    #[test]
    fn affordance_without_action_economy_fields_is_skipped() {
        // The common case: no budget/recharge fields → the FK pass does no work
        // and emits nothing even with no economy_channel_definitions authored.
        let m = Manifest {
            bundles: vec![bundle("core", &[])],
            affordance_templates: vec![aff("core/plain", "core", None, None)],
            ..Manifest::default()
        };
        let mut diags = Vec::new();
        validate_action_economy(&m, &mut diags);
        assert!(
            diags.is_empty(),
            "no-field affordance must be inert, got {diags:?}"
        );
    }

    /// Affordance row carrying only a `composes` wrapper list under test.
    fn aff_composes(template_id: &str, bundle: &str, composes: &[&str]) -> AffordanceTemplateRow {
        let mut row = aff(template_id, bundle, None, None);
        row.composes = Some(composes.iter().map(|s| (*s).to_owned()).collect());
        row
    }

    #[test]
    fn composes_resolves_to_in_scope_templates() {
        // A wrapper composing two authored children in the same bundle — clean.
        let m = Manifest {
            bundles: vec![bundle("srd", &[])],
            affordance_templates: vec![
                aff_composes("srd/multiattack", "srd", &["srd/bite", "srd/claw"]),
                aff("srd/bite", "srd", None, None),
                aff("srd/claw", "srd", None, None),
            ],
            ..Manifest::default()
        };
        let mut diags = Vec::new();
        validate_action_economy(&m, &mut diags);
        assert!(diags.is_empty(), "expected clean, got {diags:?}");
    }

    #[test]
    fn unknown_composes_child_rejected_when_closure_visible() {
        // Full closure visible (core alone — the implicit-core baseline is then
        // present) + a typo'd child id → FK reject.
        let m = Manifest {
            bundles: vec![bundle("core", &[])],
            affordance_templates: vec![
                aff_composes("core/multiattack", "core", &["core/bite", "core/typo"]),
                aff("core/bite", "core", None, None),
            ],
            ..Manifest::default()
        };
        let mut diags = Vec::new();
        validate_action_economy(&m, &mut diags);
        assert_eq!(codes(&diags), vec![MF_VAL_COMPOSES_REF]);
    }

    #[test]
    fn composes_child_trusted_when_dependency_absent() {
        // A phandelver wrapper composing an srd child, validated ALONE (srd rows
        // absent): no false-reject — same per-file leniency the FK fields use.
        let m = Manifest {
            bundles: vec![bundle("phandelver", &["srd"])],
            affordance_templates: vec![aff_composes(
                "phandelver/multiattack",
                "phandelver",
                &["srd/goblin_scimitar"],
            )],
            ..Manifest::default()
        };
        let mut diags = Vec::new();
        validate_action_economy(&m, &mut diags);
        assert!(
            diags.is_empty(),
            "cross-bundle compose trusted per-file, got {diags:?}"
        );
    }

    #[test]
    fn composes_self_reference_rejected() {
        // A wrapper composing itself is single-level-illegal (LR-7): its child
        // (itself) carries a non-empty `composes`. Subsumes the old self-cycle.
        let m = Manifest {
            bundles: vec![bundle("srd", &[])],
            affordance_templates: vec![aff_composes("srd/loop", "srd", &["srd/loop"])],
            ..Manifest::default()
        };
        let mut diags = Vec::new();
        validate_action_economy(&m, &mut diags);
        assert_eq!(codes(&diags), vec![MF_VAL_COMPOSES_NESTED]);
    }

    #[test]
    fn composes_mutual_nesting_rejected() {
        // a composes b, b composes a: each is a wrapper-as-child of the other →
        // one MF_VAL_COMPOSES_NESTED per offending edge (a→b, b→a). Subsumes the
        // old two-node cycle (no separate acyclic check — depth-1 makes a cycle
        // structurally impossible: a wrapper can never be a child).
        let m = Manifest {
            bundles: vec![bundle("srd", &[])],
            affordance_templates: vec![
                aff_composes("srd/a", "srd", &["srd/b"]),
                aff_composes("srd/b", "srd", &["srd/a"]),
            ],
            ..Manifest::default()
        };
        let mut diags = Vec::new();
        validate_action_economy(&m, &mut diags);
        assert_eq!(
            codes(&diags),
            vec![MF_VAL_COMPOSES_NESTED, MF_VAL_COMPOSES_NESTED]
        );
    }

    #[test]
    fn composes_nested_wrapper_rejected() {
        // a → {b, c}, b → d, c → d: under depth-1 (LR-7), b and c are wrappers,
        // so a composing them is rejected (a→b, a→c). b→d and c→d are legal
        // (d is a leaf). The former "diamond DAG is acyclic" admission flips to
        // rejected — composition is single-level, NOT a multi-level DAG.
        let m = Manifest {
            bundles: vec![bundle("srd", &[])],
            affordance_templates: vec![
                aff_composes("srd/a", "srd", &["srd/b", "srd/c"]),
                aff_composes("srd/b", "srd", &["srd/d"]),
                aff_composes("srd/c", "srd", &["srd/d"]),
                aff("srd/d", "srd", None, None),
            ],
            ..Manifest::default()
        };
        let mut diags = Vec::new();
        validate_action_economy(&m, &mut diags);
        assert_eq!(
            codes(&diags),
            vec![MF_VAL_COMPOSES_NESTED, MF_VAL_COMPOSES_NESTED]
        );
    }

    #[test]
    fn zero_active_scheduler_allowed_in_fragment() {
        // A non-core bundle fragment authors no active scheduler and relies on
        // core's — zero-active is a runtime invariant, not a validation error.
        let m = Manifest {
            turn_scheduler: vec![scheduler("mod/turn/disabled", "round_robin", false)],
            ..Manifest::default()
        };
        let mut diags = Vec::new();
        validate_action_economy(&m, &mut diags);
        assert!(
            diags.is_empty(),
            "fragment with no active scheduler must pass"
        );
    }

    // --- Slice E: reaction_eligibility locality (MF-VAL-238) ---

    #[test]
    fn reaction_eligibility_local_predicate_passes() {
        // A reactor-bounded predicate (reads the candidate actor's own
        // capability) is Local → the producer's per-event scan stays bounded.
        let m = Manifest {
            action_definitions: vec![action_def(
                "srd/opportunity_attack",
                "srd",
                Some(r#"actor.has_capability("srd/cap_oa")"#),
            )],
            ..Manifest::default()
        };
        let mut diags = Vec::new();
        validate_action_economy(&m, &mut diags);
        assert!(
            diags.is_empty(),
            "Local reaction predicate must pass, got {diags:?}"
        );
    }

    #[test]
    fn reaction_eligibility_global_predicate_rejected() {
        // A world-state read is Global → unbounded per-event scan → Fatal.
        let m = Manifest {
            action_definitions: vec![action_def(
                "srd/storm_ward",
                "srd",
                Some(r#"world.state("storming") == true"#),
            )],
            ..Manifest::default()
        };
        let mut diags = Vec::new();
        validate_action_economy(&m, &mut diags);
        assert_eq!(codes(&diags), vec![MF_VAL_REACTION_ELIGIBILITY_GLOBAL]);
        assert_eq!(diags[0].severity, Severity::Fatal);
    }

    #[test]
    fn reaction_eligibility_absent_is_noop() {
        // No reaction_eligibility → not a reaction candidate → no diagnostics.
        let m = Manifest {
            action_definitions: vec![action_def("srd/plain_attack", "srd", None)],
            ..Manifest::default()
        };
        let mut diags = Vec::new();
        validate_action_economy(&m, &mut diags);
        assert!(
            diags.is_empty(),
            "non-reaction action must pass, got {diags:?}"
        );
    }

    #[test]
    fn reaction_eligibility_parse_error_rejected() {
        // Garbage predicate → MF-VAL-201 parse error, the shared predicate code.
        let m = Manifest {
            action_definitions: vec![action_def(
                "srd/broken",
                "srd",
                Some("this is not @ valid ("),
            )],
            ..Manifest::default()
        };
        let mut diags = Vec::new();
        validate_action_economy(&m, &mut diags);
        assert_eq!(codes(&diags), vec![crate::MF_VAL_PREDICATE_PARSE]);
    }

    // --- Slice F-impl: reaction_trigger locality (MF-VAL-239) ---

    #[test]
    fn reaction_trigger_event_predicate_passes() {
        // A trigger over the per-event facts (`$event.*`) is bounded (Local under
        // classify_reaction_trigger) → passes; the eligibility predicate is Local.
        let m = Manifest {
            action_definitions: vec![action_def_trigger(
                "srd/opportunity_attack",
                "srd",
                r#"$event.kind == "r_move" && $event.detail.on_exit == "true""#,
            )],
            ..Manifest::default()
        };
        let mut diags = Vec::new();
        validate_action_economy(&m, &mut diags);
        assert!(
            diags.is_empty(),
            "event-bounded reaction_trigger must pass, got {diags:?}"
        );
    }

    #[test]
    fn reaction_trigger_global_predicate_rejected() {
        // A trigger reading unbounded world state is Global even under the
        // event-bounded classification → Fatal MF-VAL-239 (no escape hatch).
        let m = Manifest {
            action_definitions: vec![action_def_trigger(
                "srd/storm_react",
                "srd",
                r#"world.state("storming") == true"#,
            )],
            ..Manifest::default()
        };
        let mut diags = Vec::new();
        validate_action_economy(&m, &mut diags);
        assert_eq!(codes(&diags), vec![MF_VAL_REACTION_TRIGGER_GLOBAL]);
        assert_eq!(diags[0].severity, Severity::Fatal);
    }

    #[test]
    fn reaction_trigger_absent_is_noop() {
        // reaction_eligibility set, no reaction_trigger → trigger check is a
        // no-op (the producer provokes on every co-located event — slice E).
        let m = Manifest {
            action_definitions: vec![action_def(
                "srd/always_reacts",
                "srd",
                Some(r#"actor.has_capability("srd/cap_oa")"#),
            )],
            ..Manifest::default()
        };
        let mut diags = Vec::new();
        validate_action_economy(&m, &mut diags);
        assert!(
            diags.is_empty(),
            "absent reaction_trigger must pass, got {diags:?}"
        );
    }

    #[test]
    fn reaction_trigger_parse_error_rejected() {
        // Garbage trigger → MF-VAL-201 parse error, the shared predicate code.
        let m = Manifest {
            action_definitions: vec![action_def_trigger(
                "srd/broken_trigger",
                "srd",
                "$event.kind ==",
            )],
            ..Manifest::default()
        };
        let mut diags = Vec::new();
        validate_action_economy(&m, &mut diags);
        assert_eq!(codes(&diags), vec![crate::MF_VAL_PREDICATE_PARSE]);
    }

    /// Initiative supports resource bounds for the rolling actor.
    #[test]
    fn only_reads_actor_subject_allows_actor_resource_bound_reads() {
        let actor_expr = parse_predicate(r#"actor.resource_max("srd/hp") >= 10"#).unwrap();
        assert!(only_reads_actor_subject(&actor_expr));

        let actor_min_expr = parse_predicate(r#"actor.resource_min("srd/hp") <= 5"#).unwrap();
        assert!(only_reads_actor_subject(&actor_min_expr));

        let target_expr = parse_predicate(r#"target.resource_max("srd/hp") >= 10"#).unwrap();
        assert!(!only_reads_actor_subject(&target_expr));
    }
}
