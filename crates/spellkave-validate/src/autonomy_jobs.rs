//! Authored autonomy-job validation (CR-14 slice 1 / ADR-0041).
//!
//! Validates the authored `[[autonomy_jobs]]` manifest section.
//!
//! ## The ADR-0004 D-2 fence (authoring-time, primary)
//!
//! A subjectless autonomy job is a clock-fire: it has **no acting subject**.
//! ADR-0004 D-1 forbids the autonomy layer from any "target selection against
//! other entities"; ADR-0016 D-1 forbids the world layer from mutating
//! actor-keyed state. So a subjectless job may fire only **ambient,
//! non-actor-reaching** effects.
//!
//! ADR-0054 adds a second branch for caster-attributed lingering hazards: when a
//! job carries a stored `subject`, it may fire a small set of subject-resolved
//! actor-reaching primitives (`Damage`, `Heal`, `ApplyModifier`) with explicit
//! provenance and bounded recurrence. The old ambient branch is unchanged.
//!
//! `dsl_function` resolves to an `effect_definition` or, for ADR-0054 hazard
//! pulses, to a narrow `consequence_template` at fire time. Here we reject — at
//! authoring time, loudly — any job whose visible target is not eligible for the
//! row's subject mode. When the target is NOT visible (declared in a dependency
//! bundle validated separately, or an engine-native pulse handle like
//! `weather.advance_phase`), the token is **trusted** — the same leniency
//! `refs.rs` / `action_economy.rs` use for absent targets. The kernel's
//! fire-time eligibility check is the complete backstop that also covers
//! dependency targets this offline check cannot see through.
//!
//! [`AUTONOMY_ELIGIBLE_PRIMITIVES`] remains the ambient allowlist for drift
//! guards. The subject-bound actor-reaching set is separate and deliberately
//! narrow. Both are properties of the **closed** kernel `EffectPrimitive` set
//! (ADR-0021 D-4), not modder vocabulary — exactly like
//! `action_economy::SCHEDULER_KIND_LEGAL`, and unlike the `dsl_function` switch
//! CR-14 opens. `Spawn`, `Despawn`, and `PossessBody` stay ineligible.

use crate::{Diagnostic, Severity};
use spellkave_schema::Manifest;
use std::collections::{BTreeMap, BTreeSet};

/// `autonomy_job.job_id` is empty.
pub const MF_VAL_AUTONOMY_JOB_ID: &str = "MF-VAL-245";
/// `autonomy_job.dsl_function` is empty.
pub const MF_VAL_AUTONOMY_JOB_DSL_FUNCTION: &str = "MF-VAL-246";
/// `autonomy_job.bundle_id` uses the reserved `"runtime"` namespace (that id is
/// reserved for reducer-inserted rows via `ScheduleAutonomyJob`, never authored).
pub const MF_VAL_AUTONOMY_JOB_RUNTIME_BUNDLE: &str = "MF-VAL-247";
/// `autonomy_job.dsl_function` resolves to a non-autonomy-eligible effect (the
/// ADR-0004 D-2 fence: a clock-fire may not target/animate an actor).
pub const MF_VAL_AUTONOMY_JOB_INELIGIBLE_EFFECT: &str = "MF-VAL-248";
/// Subject-bound actor-reaching autonomy jobs must be bounded recurring jobs.
pub const MF_VAL_AUTONOMY_JOB_UNBOUNDED_SUBJECT_EFFECT: &str = "MF-VAL-249";
/// `requires_subject_alive` is meaningful only when the job stores a subject.
pub const MF_VAL_AUTONOMY_JOB_LIVENESS_WITHOUT_SUBJECT: &str = "MF-VAL-344";
/// An autonomy job must choose exactly one firing mode.
pub const MF_VAL_AUTONOMY_JOB_FIRING_MODE: &str = "MF-VAL-345";
/// `autonomy_job.subject_entity_key` (authored-hazard-pulses D-3) either
/// names no `[[entities]]` row of this manifest, or is set alongside a
/// numeric `subject` — a Class-10-style ref check (`refs.rs`'s
/// `validate_entity_tag_refs` is the closest sibling: same "resolves against
/// this manifest's own `[[entities]]` rows" shape, applied to a different
/// row kind).
pub const MF_VAL_AUTONOMY_JOB_SUBJECT_ENTITY_KEY_REF: &str = "MF-VAL-325";

/// The reserved bundle id for runtime (reducer-inserted) autonomy jobs.
const RUNTIME_BUNDLE: &str = "runtime";

/// Effect primitives an authored autonomy job may fire (ADR-0004 D-2). See the
/// module docstring: a narrow ambient allowlist, not modder vocabulary.
///
/// `pub` so the server kernel's `EffectPrimitive::autonomy_eligible_names`
/// drift-guard test can pin the two enforcement layers together (ADR-0041 D-3,
/// Consequences — the validator cannot import the kernel enum).
pub const AUTONOMY_ELIGIBLE_PRIMITIVES: &[&str] = &["InscribeTraceCluster"];

/// Subject-resolved actor-reaching primitives admitted only when an autonomy job
/// carries a stored responsible subject (ADR-0054).
pub const SUBJECT_BOUND_AUTONOMY_ACTOR_PRIMITIVES: &[&str] = &["Damage", "Heal", "ApplyModifier"];

/// Frame-write variants admitted for subject-bound caster-attributed hazard
/// consequence templates. Pinned against the server fire-time guard.
pub const AUTONOMY_ELIGIBLE_FRAME_WRITES: &[&str] =
    &["ApplyStatus", "ForEachActorInRadius", "ModifyResource"];

/// Validate the `[[autonomy_jobs]]` section.
pub fn validate_autonomy_jobs(manifest: &Manifest, diags: &mut Vec<Diagnostic>) {
    // effect_def_id -> primitives_csv, for the in-manifest eligibility check.
    let effect_primitives: BTreeMap<&str, &str> = manifest
        .effect_definitions
        .iter()
        .map(|row| (row.effect_def_id.as_str(), row.primitives_csv.as_str()))
        .collect();
    let consequence_templates: BTreeSet<&str> = manifest
        .consequence_templates
        .iter()
        .map(|row| row.template_id.as_str())
        .collect();
    // subject_entity_key (authored-hazard-pulses D-3) resolves against this
    // manifest's OWN `[[entities]]` rows only — same single-manifest scope
    // `validate_entity_tag_refs` (below, `refs.rs`-style) uses for
    // `entity_tag.entity_key`. The live commit-time resolver
    // (`resolve_manifest_entity_id`) additionally falls back to the
    // cross-bundle `manifest_authored_entity` table, which a single-manifest
    // offline validator cannot see; that gap is acceptable here for the same
    // reason `assert_place_node_exists` stays server-side only.
    let known_entity_keys: BTreeSet<&str> = manifest
        .entities
        .iter()
        .map(|row| row.entity_key.as_str())
        .collect();
    let mut frame_writes_by_template: BTreeMap<&str, Vec<&serde_json::Value>> = BTreeMap::new();
    for row in &manifest.consequence_frame_writes {
        frame_writes_by_template
            .entry(row.template_id.as_str())
            .or_default()
            .push(&row.frame_write);
    }

    for (i, row) in manifest.autonomy_jobs.iter().enumerate() {
        if row.job_id.trim().is_empty() {
            diags.push(fatal(
                MF_VAL_AUTONOMY_JOB_ID,
                &format!("autonomy_jobs[{i}].job_id"),
                "autonomy_job.job_id must not be empty".to_owned(),
            ));
        }
        if row.dsl_function.trim().is_empty() {
            diags.push(fatal(
                MF_VAL_AUTONOMY_JOB_DSL_FUNCTION,
                &format!("autonomy_jobs[{i}].dsl_function"),
                format!(
                    "autonomy_job '{}': dsl_function must not be empty",
                    row.job_id
                ),
            ));
        }
        if row.bundle_id == RUNTIME_BUNDLE {
            diags.push(fatal(
                MF_VAL_AUTONOMY_JOB_RUNTIME_BUNDLE,
                &format!("autonomy_jobs[{i}].bundle_id"),
                format!(
                    "autonomy_job '{}': bundle_id '{RUNTIME_BUNDLE}' is reserved for \
                     reducer-inserted jobs (ScheduleAutonomyJob); authored jobs must name \
                     their own bundle",
                    row.job_id
                ),
            ));
        }
        if row.subject.is_some() && !row.subject_entity_key.trim().is_empty() {
            diags.push(fatal_with_hint(
                MF_VAL_AUTONOMY_JOB_SUBJECT_ENTITY_KEY_REF,
                &format!("autonomy_jobs[{i}].subject_entity_key"),
                format!(
                    "autonomy_job '{}': sets both subject and subject_entity_key",
                    row.job_id
                ),
                "author only one of subject (a numeric entity_id) or subject_entity_key (an [[entities]] row's entity_key)",
            ));
        } else if !row.subject_entity_key.trim().is_empty()
            && !known_entity_keys.contains(row.subject_entity_key.as_str())
        {
            diags.push(fatal_with_hint(
                MF_VAL_AUTONOMY_JOB_SUBJECT_ENTITY_KEY_REF,
                &format!("autonomy_jobs[{i}].subject_entity_key"),
                format!(
                    "autonomy_job '{}': subject_entity_key '{}' does not resolve to any [[entities]] row of this manifest",
                    row.job_id, row.subject_entity_key
                ),
                "author an [[entities]] row with a matching entity_key, or set subject to an existing entity_id directly",
            ));
        }
        if row.requires_subject_alive && !row_has_subject(row) {
            diags.push(fatal_with_hint(
                MF_VAL_AUTONOMY_JOB_LIVENESS_WITHOUT_SUBJECT,
                &format!("autonomy_jobs[{i}].requires_subject_alive"),
                format!(
                    "autonomy_job '{}': requires_subject_alive requires a stored subject",
                    row.job_id
                ),
                "set subject to the responsible caster entity_id or leave requires_subject_alive false",
            ));
        }
        if !has_valid_firing_mode(row) {
            diags.push(fatal_with_hint(
                MF_VAL_AUTONOMY_JOB_FIRING_MODE,
                &format!("autonomy_jobs[{i}]"),
                format!(
                    "autonomy_job '{}': choose exactly one firing mode: next_fire_at_ms or fire_at_boundary with owner_actor",
                    row.job_id
                ),
                "absolute jobs require next_fire_at_ms and leave fire_at_boundary empty with owner_actor = 0; boundary jobs omit next_fire_at_ms and require both fire_at_boundary and owner_actor",
            ));
        }

        // ADR-0004 D-2 fence — only when the named effect is visible here.
        if let Some(primitives_csv) = effect_primitives.get(row.dsl_function.as_str()) {
            let primitives = primitive_tokens(primitives_csv);
            let has_subject = row_has_subject(row);
            let ineligible: Vec<&str> = primitives
                .iter()
                .copied()
                .filter(|p| !autonomy_eligible_for_subject_mode(p, has_subject))
                .collect();
            if !ineligible.is_empty() {
                diags.push(Diagnostic {
                    code: MF_VAL_AUTONOMY_JOB_INELIGIBLE_EFFECT.to_owned(),
                    severity: Severity::Fatal,
                    message: format!(
                        "autonomy_job '{}': dsl_function '{}' resolves to effect with \
                         non-autonomy-eligible primitive(s) [{}] for {} autonomy (ADR-0004 D-2 / \
                         ADR-0054). Eligible in this mode: [{}].",
                        row.job_id,
                        row.dsl_function,
                        ineligible.join(", "),
                        if has_subject { "subject-bound" } else { "ambient" },
                        eligible_primitives_for_subject_mode(has_subject).join(", "),
                    ),
                    path: format!("autonomy_jobs[{i}].dsl_function"),
                    span: None,
                    hint: Some(
                        "ambient jobs may only fire InscribeTraceCluster; subject-bound jobs may also fire Damage, Heal, or ApplyModifier"
                            .to_owned(),
                    ),
                    details: Some(serde_json::json!({
                        "dsl_function": row.dsl_function,
                        "has_subject": has_subject,
                        "ineligible_primitives": ineligible,
                        "eligible_primitives": eligible_primitives_for_subject_mode(has_subject),
                    })),
                });
            }
            push_unbounded_subject_diag_if_needed(
                row.frequency_ms,
                row.repeats_remaining,
                has_subject,
                !subject_bound_actor_primitives(&primitives).is_empty(),
                i,
                &row.job_id,
                &row.dsl_function,
                row.subject,
                diags,
            );
        } else if consequence_templates.contains(row.dsl_function.as_str()) {
            let has_subject = row_has_subject(row);
            let writes = frame_writes_by_template
                .get(row.dsl_function.as_str())
                .cloned()
                .unwrap_or_default();
            let ineligible = ineligible_autonomy_frame_writes(&writes);
            if !has_subject || !ineligible.is_empty() {
                diags.push(Diagnostic {
                    code: MF_VAL_AUTONOMY_JOB_INELIGIBLE_EFFECT.to_owned(),
                    severity: Severity::Fatal,
                    message: format!(
                        "autonomy_job '{}': dsl_function '{}' resolves to consequence_template \
                         that is not eligible for {} autonomy (ADR-0054). Eligible frame writes \
                         in this mode: [{}].",
                        row.job_id,
                        row.dsl_function,
                        if has_subject { "subject-bound" } else { "ambient" },
                        if has_subject {
                            AUTONOMY_ELIGIBLE_FRAME_WRITES.join(", ")
                        } else {
                            "none".to_owned()
                        },
                    ),
                    path: format!("autonomy_jobs[{i}].dsl_function"),
                    span: None,
                    hint: Some(
                        "only subject-bound caster-attributed hazards may fire consequence templates; the template may contain ApplyStatus, ForEachActorInRadius, and ModifyResource only"
                            .to_owned(),
                    ),
                    details: Some(serde_json::json!({
                        "dsl_function": row.dsl_function,
                        "has_subject": has_subject,
                        "ineligible_frame_writes": ineligible,
                    })),
                });
            }
            push_unbounded_subject_diag_if_needed(
                row.frequency_ms,
                row.repeats_remaining,
                has_subject,
                writes
                    .iter()
                    .any(|write| frame_write_tree_contains_actor_reaching_hazard_write(write)),
                i,
                &row.job_id,
                &row.dsl_function,
                row.subject,
                diags,
            );
        }
    }
}

/// Whether `row` is subject-bound for the eligibility fence, considering
/// BOTH the numeric `subject` column and the manifest-authoring
/// `subject_entity_key` alternative (authored-hazard-pulses D-3). The offline
/// validator never resolves `subject_entity_key` to a numeric id (that
/// resolves later). Either form makes the row subject-bound for this check.
fn row_has_subject(row: &spellkave_schema::AutonomyJobRow) -> bool {
    row.subject.is_some() || !row.subject_entity_key.trim().is_empty()
}

fn has_valid_firing_mode(row: &spellkave_schema::AutonomyJobRow) -> bool {
    let absolute = row.next_fire_at_ms.is_some()
        && row.fire_at_boundary.trim().is_empty()
        && row.owner_actor == 0;
    let boundary = row.next_fire_at_ms.is_none()
        && !row.fire_at_boundary.trim().is_empty()
        && row.owner_actor != 0;
    absolute || boundary
}

fn primitive_tokens(primitives_csv: &str) -> Vec<&str> {
    primitives_csv
        .split(',')
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .collect()
}

fn autonomy_eligible_for_subject_mode(primitive: &str, has_subject: bool) -> bool {
    AUTONOMY_ELIGIBLE_PRIMITIVES.contains(&primitive)
        || (has_subject && SUBJECT_BOUND_AUTONOMY_ACTOR_PRIMITIVES.contains(&primitive))
}

fn eligible_primitives_for_subject_mode(has_subject: bool) -> Vec<&'static str> {
    let mut primitives = AUTONOMY_ELIGIBLE_PRIMITIVES.to_vec();
    if has_subject {
        primitives.extend_from_slice(SUBJECT_BOUND_AUTONOMY_ACTOR_PRIMITIVES);
    }
    primitives
}

fn subject_bound_actor_primitives<'a>(primitives: &'a [&'a str]) -> Vec<&'a str> {
    primitives
        .iter()
        .copied()
        .filter(|p| SUBJECT_BOUND_AUTONOMY_ACTOR_PRIMITIVES.contains(p))
        .collect()
}

fn ineligible_autonomy_frame_writes(writes: &[&serde_json::Value]) -> Vec<String> {
    let mut ineligible = Vec::new();
    for write in writes {
        collect_ineligible_autonomy_frame_writes(write, &mut ineligible);
    }
    ineligible
}

fn collect_ineligible_autonomy_frame_writes(
    value: &serde_json::Value,
    ineligible: &mut Vec<String>,
) {
    let serde_json::Value::Object(map) = value else {
        ineligible.push(value.as_str().unwrap_or("<non-object>").to_owned());
        return;
    };
    if map.len() != 1 {
        ineligible.push("<multi-variant-object>".to_owned());
        return;
    }
    let (variant, inner) = map.iter().next().expect("len checked");
    match variant.as_str() {
        "ApplyStatus" => {}
        "ModifyResource" => {}
        "ForEachActorInRadius" => {
            let serde_json::Value::Object(fields) = inner else {
                ineligible.push("ForEachActorInRadius".to_owned());
                return;
            };
            let Some(serde_json::Value::Array(body)) = fields.get("body") else {
                ineligible.push("ForEachActorInRadius".to_owned());
                return;
            };
            for leaf in body {
                collect_ineligible_autonomy_frame_writes(leaf, ineligible);
            }
        }
        other => ineligible.push(other.to_owned()),
    }
}

fn frame_write_tree_contains_actor_reaching_hazard_write(value: &serde_json::Value) -> bool {
    let serde_json::Value::Object(map) = value else {
        return false;
    };
    if map.len() != 1 {
        return false;
    }
    let (variant, inner) = map.iter().next().expect("len checked");
    match variant.as_str() {
        "ApplyStatus" | "ModifyResource" => true,
        "ForEachActorInRadius" => {
            let serde_json::Value::Object(fields) = inner else {
                return false;
            };
            let Some(serde_json::Value::Array(body)) = fields.get("body") else {
                return false;
            };
            body.iter()
                .any(frame_write_tree_contains_actor_reaching_hazard_write)
        }
        _ => false,
    }
}

#[allow(clippy::too_many_arguments)]
fn push_unbounded_subject_diag_if_needed(
    frequency_ms: u64,
    repeats_remaining: Option<u64>,
    has_subject: bool,
    actor_reaching: bool,
    i: usize,
    job_id: &str,
    dsl_function: &str,
    subject: Option<u64>,
    diags: &mut Vec<Diagnostic>,
) {
    if has_subject && frequency_ms > 0 && repeats_remaining.is_none() && actor_reaching {
        diags.push(Diagnostic {
            code: MF_VAL_AUTONOMY_JOB_UNBOUNDED_SUBJECT_EFFECT.to_owned(),
            severity: Severity::Fatal,
            message: format!(
                "autonomy_job '{job_id}': subject-bound actor-reaching effect '{dsl_function}' \
                 must set repeats_remaining for bounded recurrence"
            ),
            path: format!("autonomy_jobs[{i}].repeats_remaining"),
            span: None,
            hint: Some(
                "set repeats_remaining to a finite pulse count for caster-attributed hazards"
                    .to_owned(),
            ),
            details: Some(serde_json::json!({
                "dsl_function": dsl_function,
                "subject": subject,
                "actor_reaching": actor_reaching,
            })),
        });
    }
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

fn fatal_with_hint(code: &str, path: &str, message: String, hint: &str) -> Diagnostic {
    Diagnostic {
        code: code.to_owned(),
        severity: Severity::Fatal,
        message,
        path: path.to_owned(),
        span: None,
        hint: Some(hint.to_owned()),
        details: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use spellkave_schema::{
        AutonomyJobRow, ConsequenceFrameWriteRow, ConsequenceTemplateRow, EffectDefinitionRow,
        EntityKindRow, EntityRow,
    };

    fn manifest_with(jobs: Vec<AutonomyJobRow>, effects: Vec<EffectDefinitionRow>) -> Manifest {
        Manifest {
            autonomy_jobs: jobs,
            effect_definitions: effects,
            ..Manifest::default()
        }
    }

    /// Same shape as `manifest_with`, plus `[[entities]]` rows —
    /// `subject_entity_key` (authored-hazard-pulses D-3) resolves against
    /// these.
    fn manifest_with_entities(
        jobs: Vec<AutonomyJobRow>,
        effects: Vec<EffectDefinitionRow>,
        entities: Vec<EntityRow>,
    ) -> Manifest {
        Manifest {
            autonomy_jobs: jobs,
            effect_definitions: effects,
            entities,
            ..Manifest::default()
        }
    }

    fn entity(entity_key: &str) -> EntityRow {
        EntityRow {
            entity_key: entity_key.into(),
            bundle_id: "mech_burning_room".into(),
            entity_def_id: String::new(),
            name: entity_key.into(),
            kind: EntityKindRow::Npc,
            place_node_id: String::new(),
            quantity: 1,
            owned_by_entity_key: String::new(),
            description: String::new(),
        }
    }

    fn ambient_job(dsl_function: &str) -> AutonomyJobRow {
        AutonomyJobRow {
            job_id: "core.ambient_pulse".into(),
            bundle_id: "core".into(),
            frequency_ms: 10,
            next_fire_at_ms: Some(0),
            fire_at_boundary: String::new(),
            owner_actor: 0,
            repeats_remaining: None,
            subject: None,
            subject_entity_key: String::new(),
            anchor_place_node_id: None,
            requires_subject_alive: false,
            dsl_function: dsl_function.into(),
            payload_json: String::new(),
            priority: 0,
            enabled: true,
            source: "manifest".into(),
            description: String::new(),
            notable: false,
        }
    }

    fn subject_job(dsl_function: &str) -> AutonomyJobRow {
        AutonomyJobRow {
            subject: Some(42),
            repeats_remaining: Some(3),
            ..ambient_job(dsl_function)
        }
    }

    /// Manifest-authoring alternative to `subject_job` (authored-hazard-pulses
    /// D-3): names the subject by `entity_key` instead of a numeric id.
    fn subject_entity_key_job(dsl_function: &str, entity_key: &str) -> AutonomyJobRow {
        AutonomyJobRow {
            subject_entity_key: entity_key.into(),
            repeats_remaining: Some(3),
            ..ambient_job(dsl_function)
        }
    }

    fn effect(effect_def_id: &str, primitives_csv: &str) -> EffectDefinitionRow {
        EffectDefinitionRow {
            effect_def_id: effect_def_id.into(),
            bundle_id: "core".into(),
            primitives_csv: primitives_csv.into(),
            parameters_schema_json: "{}".into(),
            description: String::new(),
        }
    }

    fn consequence(template_id: &str) -> ConsequenceTemplateRow {
        ConsequenceTemplateRow {
            template_id: template_id.into(),
            bundle_id: "core".into(),
            effect_refs_csv: String::new(),
            traces_json: "[]".into(),
            description: String::new(),
        }
    }

    #[test]
    fn authored_unit_seal_break_is_never_autonomy_eligible() {
        let operation = serde_json::json!("BreakPlaceSeal");
        assert_eq!(
            ineligible_autonomy_frame_writes(&[&operation]),
            ["BreakPlaceSeal"]
        );
    }

    fn frame_write(template_id: &str, frame_write: serde_json::Value) -> ConsequenceFrameWriteRow {
        ConsequenceFrameWriteRow {
            template_id: template_id.into(),
            ordinal: 0,
            frame_write,
        }
    }

    fn manifest_with_consequence(
        jobs: Vec<AutonomyJobRow>,
        templates: Vec<ConsequenceTemplateRow>,
        frame_writes: Vec<ConsequenceFrameWriteRow>,
    ) -> Manifest {
        Manifest {
            autonomy_jobs: jobs,
            consequence_templates: templates,
            consequence_frame_writes: frame_writes,
            ..Manifest::default()
        }
    }

    fn hazard_fan_write() -> serde_json::Value {
        serde_json::json!({
            "ForEachActorInRadius": {
                "center_ref": "ActorSpatialRef",
                "shape_id": "core/area/node_ball",
                "shape_params_json": "{\"ball_radius\": 0}",
                "predicate_source": "",
                "body": [
                    {
                        "ModifyResource": {
                            "actor": "Actor",
                            "resource": "core/hp",
                            "delta_source": "-1"
                        }
                    }
                ]
            }
        })
    }

    fn condition_hazard_fan_write() -> serde_json::Value {
        serde_json::json!({
            "ForEachActorInRadius": {
                "center_ref": "ActorSpatialRef",
                "shape_id": "core/area/node_ball",
                "shape_params_json": "{\"ball_radius\": 0}",
                "predicate_source": "roll_under(d20, 13)",
                "body": [
                    {
                        "ApplyStatus": {
                            "target_kind": "entity",
                            "target_ref": "Actor",
                            "condition_definition_id": "core/poisoned"
                        }
                    }
                ]
            }
        })
    }

    fn codes(diags: &[Diagnostic]) -> Vec<String> {
        diags.iter().map(|d| d.code.clone()).collect()
    }

    #[test]
    fn ambient_inscribe_job_is_eligible() {
        let manifest = manifest_with(
            vec![ambient_job("core/ambient_trace")],
            vec![effect("core/ambient_trace", "InscribeTraceCluster")],
        );
        let mut diags = Vec::new();
        validate_autonomy_jobs(&manifest, &mut diags);
        assert!(
            diags.is_empty(),
            "ambient InscribeTraceCluster job must pass: {diags:?}"
        );
    }

    #[test]
    fn rejects_missing_or_mixed_firing_modes() {
        let mut missing = ambient_job("core/ambient_trace");
        missing.next_fire_at_ms = None;
        let mut mixed = ambient_job("core/ambient_trace");
        mixed.fire_at_boundary = "TurnEnd".into();
        mixed.owner_actor = 42;
        let manifest = manifest_with(
            vec![missing, mixed],
            vec![effect("core/ambient_trace", "InscribeTraceCluster")],
        );
        let mut diags = Vec::new();
        validate_autonomy_jobs(&manifest, &mut diags);
        assert_eq!(
            codes(&diags)
                .iter()
                .filter(|code| *code == MF_VAL_AUTONOMY_JOB_FIRING_MODE)
                .count(),
            2
        );
    }

    #[test]
    fn boundary_firing_mode_is_eligible() {
        let mut job = ambient_job("core/ambient_trace");
        job.next_fire_at_ms = None;
        job.fire_at_boundary = "TurnEnd".into();
        job.owner_actor = 42;
        let manifest = manifest_with(
            vec![job],
            vec![effect("core/ambient_trace", "InscribeTraceCluster")],
        );
        let mut diags = Vec::new();
        validate_autonomy_jobs(&manifest, &mut diags);
        assert!(diags.is_empty(), "boundary job must pass: {diags:?}");
    }

    #[test]
    fn ambient_actor_targeting_job_is_rejected() {
        // The D-2 fence: a subjectless job firing a Damage effect targets an actor.
        let manifest = manifest_with(
            vec![ambient_job("core/zap")],
            vec![effect("core/zap", "Damage")],
        );
        let mut diags = Vec::new();
        validate_autonomy_jobs(&manifest, &mut diags);
        assert!(codes(&diags).contains(&MF_VAL_AUTONOMY_JOB_INELIGIBLE_EFFECT.to_owned()));
    }

    #[test]
    fn subject_bound_damage_job_is_eligible_when_bounded() {
        let manifest = manifest_with(
            vec![subject_job("core/cloudkill_pulse")],
            vec![effect("core/cloudkill_pulse", "Damage")],
        );
        let mut diags = Vec::new();
        validate_autonomy_jobs(&manifest, &mut diags);
        assert!(
            diags.is_empty(),
            "subject-bound bounded Damage job must pass: {diags:?}"
        );
    }

    #[test]
    fn subject_entity_key_resolving_to_an_entity_is_eligible() {
        let manifest = manifest_with_entities(
            vec![subject_entity_key_job(
                "core/cloudkill_pulse",
                "mech/burning_room/fire",
            )],
            vec![effect("core/cloudkill_pulse", "Damage")],
            vec![entity("mech/burning_room/fire")],
        );
        let mut diags = Vec::new();
        validate_autonomy_jobs(&manifest, &mut diags);
        assert!(
            diags.is_empty(),
            "subject_entity_key naming an authored entity must pass: {diags:?}"
        );
    }

    #[test]
    fn subject_entity_key_naming_no_entity_is_rejected() {
        let manifest = manifest_with_entities(
            vec![subject_entity_key_job(
                "core/cloudkill_pulse",
                "mech/burning_room/missing_fire",
            )],
            vec![effect("core/cloudkill_pulse", "Damage")],
            vec![entity("mech/burning_room/fire")],
        );
        let mut diags = Vec::new();
        validate_autonomy_jobs(&manifest, &mut diags);
        assert!(
            codes(&diags).contains(&MF_VAL_AUTONOMY_JOB_SUBJECT_ENTITY_KEY_REF.to_owned()),
            "unresolvable subject_entity_key must be rejected: {diags:?}"
        );
    }

    #[test]
    fn subject_and_subject_entity_key_both_set_is_rejected() {
        let mut job = subject_entity_key_job("core/cloudkill_pulse", "mech/burning_room/fire");
        job.subject = Some(42);
        let manifest = manifest_with_entities(
            vec![job],
            vec![effect("core/cloudkill_pulse", "Damage")],
            vec![entity("mech/burning_room/fire")],
        );
        let mut diags = Vec::new();
        validate_autonomy_jobs(&manifest, &mut diags);
        assert!(
            codes(&diags).contains(&MF_VAL_AUTONOMY_JOB_SUBJECT_ENTITY_KEY_REF.to_owned()),
            "both subject and subject_entity_key set must be rejected: {diags:?}"
        );
    }

    #[test]
    fn subject_bound_consequence_template_hazard_is_eligible_when_bounded() {
        let manifest = manifest_with_consequence(
            vec![subject_job("core/cloudkill_pulse")],
            vec![consequence("core/cloudkill_pulse")],
            vec![frame_write("core/cloudkill_pulse", hazard_fan_write())],
        );
        let mut diags = Vec::new();
        validate_autonomy_jobs(&manifest, &mut diags);
        assert!(
            diags.is_empty(),
            "subject-bound bounded ForEachActorInRadius hazard must pass: {diags:?}"
        );
    }

    #[test]
    fn subject_bound_consequence_template_condition_hazard_is_eligible_when_bounded() {
        let manifest = manifest_with_consequence(
            vec![subject_job("core/stinking_cloud_pulse")],
            vec![consequence("core/stinking_cloud_pulse")],
            vec![frame_write(
                "core/stinking_cloud_pulse",
                condition_hazard_fan_write(),
            )],
        );
        let mut diags = Vec::new();
        validate_autonomy_jobs(&manifest, &mut diags);
        assert!(
            diags.is_empty(),
            "subject-bound bounded ForEachActorInRadius -> ApplyStatus hazard must pass: {diags:?}"
        );
    }

    #[test]
    fn ambient_consequence_template_hazard_is_rejected() {
        let manifest = manifest_with_consequence(
            vec![ambient_job("core/cloudkill_pulse")],
            vec![consequence("core/cloudkill_pulse")],
            vec![frame_write("core/cloudkill_pulse", hazard_fan_write())],
        );
        let mut diags = Vec::new();
        validate_autonomy_jobs(&manifest, &mut diags);
        assert!(codes(&diags).contains(&MF_VAL_AUTONOMY_JOB_INELIGIBLE_EFFECT.to_owned()));
    }

    #[test]
    fn subject_bound_consequence_template_must_stay_in_hazard_slice() {
        let manifest = manifest_with_consequence(
            vec![subject_job("core/delete_zone")],
            vec![consequence("core/delete_zone")],
            vec![frame_write(
                "core/delete_zone",
                serde_json::json!({"DeleteEntity": {"entity": "Actor"}}),
            )],
        );
        let mut diags = Vec::new();
        validate_autonomy_jobs(&manifest, &mut diags);
        assert!(codes(&diags).contains(&MF_VAL_AUTONOMY_JOB_INELIGIBLE_EFFECT.to_owned()));
        assert!(
            diags.iter().any(|d| d.message.contains("ApplyStatus")),
            "eligible-frame-write diagnostic should list ApplyStatus: {diags:?}"
        );
    }

    #[test]
    fn subject_bound_consequence_template_actor_job_must_be_bounded() {
        let mut job = subject_job("core/cloudkill_pulse");
        job.repeats_remaining = None;
        let manifest = manifest_with_consequence(
            vec![job],
            vec![consequence("core/cloudkill_pulse")],
            vec![frame_write("core/cloudkill_pulse", hazard_fan_write())],
        );
        let mut diags = Vec::new();
        validate_autonomy_jobs(&manifest, &mut diags);
        assert!(codes(&diags).contains(&MF_VAL_AUTONOMY_JOB_UNBOUNDED_SUBJECT_EFFECT.to_owned()));
    }

    #[test]
    fn subject_bound_consequence_template_condition_job_must_be_bounded() {
        let mut job = subject_job("core/stinking_cloud_pulse");
        job.repeats_remaining = None;
        let manifest = manifest_with_consequence(
            vec![job],
            vec![consequence("core/stinking_cloud_pulse")],
            vec![frame_write(
                "core/stinking_cloud_pulse",
                condition_hazard_fan_write(),
            )],
        );
        let mut diags = Vec::new();
        validate_autonomy_jobs(&manifest, &mut diags);
        assert!(codes(&diags).contains(&MF_VAL_AUTONOMY_JOB_UNBOUNDED_SUBJECT_EFFECT.to_owned()));
    }

    #[test]
    fn subject_bound_actor_job_must_be_bounded_when_recurring() {
        let mut job = subject_job("core/cloudkill_pulse");
        job.frequency_ms = 10;
        job.repeats_remaining = None;
        let manifest = manifest_with(vec![job], vec![effect("core/cloudkill_pulse", "Damage")]);
        let mut diags = Vec::new();
        validate_autonomy_jobs(&manifest, &mut diags);
        assert!(codes(&diags).contains(&MF_VAL_AUTONOMY_JOB_UNBOUNDED_SUBJECT_EFFECT.to_owned()));
    }

    #[test]
    fn subject_bound_possess_body_is_still_rejected() {
        let manifest = manifest_with(
            vec![subject_job("core/possession")],
            vec![effect("core/possession", "PossessBody")],
        );
        let mut diags = Vec::new();
        validate_autonomy_jobs(&manifest, &mut diags);
        assert!(codes(&diags).contains(&MF_VAL_AUTONOMY_JOB_INELIGIBLE_EFFECT.to_owned()));
    }

    #[test]
    fn spawn_job_is_rejected_param_resolved_actor_reach() {
        // Spawn is param-resolved (carries blueprint_id) — the offline fence
        // rejects it here; the kernel fire-time check (slice 2) is the backstop.
        let manifest = manifest_with(
            vec![ambient_job("core/conjure")],
            vec![effect("core/conjure", "Spawn")],
        );
        let mut diags = Vec::new();
        validate_autonomy_jobs(&manifest, &mut diags);
        assert!(codes(&diags).contains(&MF_VAL_AUTONOMY_JOB_INELIGIBLE_EFFECT.to_owned()));
    }

    #[test]
    fn subject_bound_spawn_job_is_still_rejected_param_resolved_actor_reach() {
        let manifest = manifest_with(
            vec![subject_job("core/conjure")],
            vec![effect("core/conjure", "Spawn")],
        );
        let mut diags = Vec::new();
        validate_autonomy_jobs(&manifest, &mut diags);
        assert!(codes(&diags).contains(&MF_VAL_AUTONOMY_JOB_INELIGIBLE_EFFECT.to_owned()));
    }

    #[test]
    fn requires_subject_alive_requires_subject() {
        let mut job = ambient_job("core/ambient_trace");
        job.requires_subject_alive = true;
        let manifest = manifest_with(
            vec![job],
            vec![effect("core/ambient_trace", "InscribeTraceCluster")],
        );
        let mut diags = Vec::new();
        validate_autonomy_jobs(&manifest, &mut diags);
        assert!(codes(&diags).contains(&MF_VAL_AUTONOMY_JOB_LIVENESS_WITHOUT_SUBJECT.to_owned()));
    }

    #[test]
    fn unresolvable_dsl_function_is_trusted() {
        // Engine-native pulse handle (no effect_definition) — trusted, not flagged.
        let manifest = manifest_with(vec![ambient_job("weather.advance_phase")], vec![]);
        let mut diags = Vec::new();
        validate_autonomy_jobs(&manifest, &mut diags);
        assert!(
            diags.is_empty(),
            "unresolvable handle must be trusted: {diags:?}"
        );
    }

    #[test]
    fn empty_job_id_and_dsl_function_are_rejected() {
        let mut job = ambient_job("");
        job.job_id = String::new();
        let manifest = manifest_with(vec![job], vec![]);
        let mut diags = Vec::new();
        validate_autonomy_jobs(&manifest, &mut diags);
        let c = codes(&diags);
        assert!(c.contains(&MF_VAL_AUTONOMY_JOB_ID.to_owned()));
        assert!(c.contains(&MF_VAL_AUTONOMY_JOB_DSL_FUNCTION.to_owned()));
    }

    #[test]
    fn reserved_runtime_bundle_is_rejected() {
        let mut job = ambient_job("core/ambient_trace");
        job.bundle_id = "runtime".into();
        let manifest = manifest_with(
            vec![job],
            vec![effect("core/ambient_trace", "InscribeTraceCluster")],
        );
        let mut diags = Vec::new();
        validate_autonomy_jobs(&manifest, &mut diags);
        assert!(codes(&diags).contains(&MF_VAL_AUTONOMY_JOB_RUNTIME_BUNDLE.to_owned()));
    }
}
