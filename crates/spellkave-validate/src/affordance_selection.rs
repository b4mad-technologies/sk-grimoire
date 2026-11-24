//! Affordance-selection ambiguity diagnostic (P3 — retire-priority-eligibility-select,
//! Decision 4 / task 4).
//!
//! ADR-0021 D-2 Postscript (2026-06-28) retired the author-typed
//! `affordance_template.priority` knob: per-verb selection disambiguates by
//! **eligibility** (niche + predicate) plus a deterministic, total tie-break on
//! the stable `template_id`. A genuine overlap — two `affordance_template` rows
//! simultaneously eligible for ONE resolved action at ONE niche — used to be
//! silently masked by the priority integer. It is now surfaced here as an
//! **author-visible diagnostic**, realising C-2's "validation-time ambiguity
//! rejection" intent on the affordance axis.
//!
//! Severity is **Warning** in v1: static disjointness of arbitrary DSL predicates
//! is undecidable in general, so this flags every same-`(niche_id,
//! resolved-action_id)` group with >=2 candidates rather than risk a
//! false-positive build break. The runtime `template_id` tie-break guarantees
//! determinism regardless — a warned-but-unfixed overlap is still deterministic,
//! just possibly not the author's intent. (Open Question 1: promote Warning ->
//! Error once a trusted disjointness checker lands.)

use std::collections::BTreeMap;

use spellkave_dsl::count_static_draws;
use spellkave_schema::Manifest;

use crate::{Diagnostic, Severity};

pub const MF_VAL_AFFORDANCE_SELECTION_AMBIGUITY: &str = "MF-VAL-341";

/// Flag `(niche_id, resolved manipulation_action_id)` groups carrying >=2
/// `affordance_template` rows — the only places the retired `priority` knob could
/// have been load-bearing. Move/sense affordances (whose outcome carries no
/// `manipulation_action_id`) are not selection-priority candidates and are
/// skipped; an affordance with an empty `niche_id` is skipped likewise.
pub fn validate_affordance_selection_ambiguity(manifest: &Manifest, diags: &mut Vec<Diagnostic>) {
    // outcome_template_id -> resolved manipulation_action_id.
    let outcome_action: BTreeMap<&str, &str> = manifest
        .outcome_templates
        .iter()
        .filter_map(|o| {
            let action = o.manipulation_action_id.trim();
            (!action.is_empty()).then_some((o.outcome_template_id.as_str(), action))
        })
        .collect();

    // Group affordances by (niche_id, resolved-action_id). BTreeMap → stable,
    // deterministic diagnostic order.
    let mut groups: BTreeMap<(&str, &str), Vec<&str>> = BTreeMap::new();
    for t in &manifest.affordance_templates {
        let niche = t.niche_id.trim();
        if niche.is_empty() {
            continue;
        }
        let Some(action) = outcome_action.get(t.outcome_template_id.as_str()) else {
            continue;
        };
        groups
            .entry((niche, *action))
            .or_default()
            .push(t.template_id.as_str());
    }

    for ((niche, action), templates) in groups {
        if templates.len() < 2 {
            continue;
        }
        diags.push(Diagnostic {
            code: MF_VAL_AFFORDANCE_SELECTION_AMBIGUITY.to_owned(),
            severity: Severity::Warning,
            message: format!(
                "{} affordance_templates resolve action '{action}' at niche '{niche}' ({}). \
                 Selection now disambiguates by eligibility + a deterministic template_id \
                 tie-break (no author priority); if more than one is simultaneously eligible \
                 the tie-break — not author intent — decides which fires.",
                templates.len(),
                templates.join(", "),
            ),
            path: format!("affordance_templates[niche_id={niche},action_id={action}]"),
            span: None,
            hint: Some(
                "make the candidates' predicates mutually exclusive (only one ever passes), or \
                 confirm they are interchangeable and rely on the stable template_id order"
                    .to_owned(),
            ),
            details: None,
        });
    }
}

/// #1556 root cause — the promised-but-never-built half of the ADR-0021 D-2
/// Postscript ambiguity diagnostic. `MF_VAL_AFFORDANCE_SELECTION_AMBIGUITY`
/// above groups by `(niche_id, action_id)`, so it only ever catches an
/// overlap between candidates that share a niche. The act-time selection loop
/// considers every affordance-bearing niche in scope for the requested action
/// and evaluates each
/// candidate's predicate with its own fresh draw, first pass wins. Two
/// affordance_templates on DIFFERENT niches that both resolve the same
/// `manipulation_action_id` are therefore both live candidates for one act —
/// invisible to the niche-scoped check above.
///
/// That fan-out is harmless when candidates are deterministic (evaluates the
/// same either way) or mutually exclusive by eligibility (only one is ever a
/// candidate at once). It becomes a live authoring bug when **every**
/// candidate in the group carries its own **stochastic** draw (a `d20`, dice
/// roll, or roll_table pull in its predicate): one act then samples N
/// independent rolls and keeps the first that passes, silently inflating the
/// pass rate above what any single candidate's predicate implies — exactly
/// the mechanism that produced #1556's biased provenance (`srd/attack/melee`
/// resolved by both `srd/attack/melee/affordance` and
/// `phandelver/hero/reactive_strike/affordance`, at two different niches).
///
/// Severity is **Warning**, matching the MF_VAL_AFFORDANCE_SELECTION_AMBIGUITY
/// ratchet posture (#1510/#1359 precedent): this is a static approximation
/// (draw-count only, not disjointness) and a new Fatal lint must not redden
/// an in-flight corpus. Promote once the corpus sweeps clean.
pub const MF_VAL_STOCHASTIC_AFFORDANCE_OVERLAP: &str = "MF-VAL-273";

/// Flag resolved-action groups (ACROSS niches, unlike
/// `validate_affordance_selection_ambiguity`) carrying >=2 `affordance_template`
/// rows whose predicates EACH contain at least one stochastic draw
/// (`count_static_draws(predicate) > 0`). An affordance with an empty or
/// non-parsing predicate source is treated as non-stochastic (its own parse
/// errors are reported by `MF_VAL_PREDICATE_PARSE`, not duplicated here).
pub fn validate_stochastic_affordance_overlap(manifest: &Manifest, diags: &mut Vec<Diagnostic>) {
    let macros = crate::dsl_macros::dsl_macro_compiler_definitions(manifest);
    // outcome_template_id -> resolved manipulation_action_id (mirrors
    // validate_affordance_selection_ambiguity's lookup).
    let outcome_action: BTreeMap<&str, &str> = manifest
        .outcome_templates
        .iter()
        .filter_map(|o| {
            let action = o.manipulation_action_id.trim();
            (!action.is_empty()).then_some((o.outcome_template_id.as_str(), action))
        })
        .collect();

    // Group STOCHASTIC affordances by resolved action_id ONLY — no niche key,
    // since the act-time fan-out crosses niches.
    let mut groups: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for t in &manifest.affordance_templates {
        let Some(action) = outcome_action.get(t.outcome_template_id.as_str()) else {
            continue;
        };
        let source = t.predicate_source.trim();
        if source.is_empty() {
            continue;
        }
        let Ok(expr) = crate::dsl_macros::parse_predicate(source, &macros) else {
            continue;
        };
        if count_static_draws(&expr) == 0 {
            continue;
        }
        groups
            .entry(*action)
            .or_default()
            .push(t.template_id.as_str());
    }

    for (action, templates) in groups {
        if templates.len() < 2 {
            continue;
        }
        diags.push(Diagnostic {
            code: MF_VAL_STOCHASTIC_AFFORDANCE_OVERLAP.to_owned(),
            severity: Severity::Warning,
            message: format!(
                "{} affordance_templates resolve action '{action}' with an independently \
                 stochastic predicate each ({}). At act time the candidate for this action is \
                 selected by evaluating every eligible candidate with its own fresh draw and \
                 keeping the first that passes, so one act can sample multiple independent \
                 rolls instead of the single roll each predicate individually implies, and only \
                 the winning candidate's draw is recorded.",
                templates.len(),
                templates.join(", "),
            ),
            path: format!("affordance_templates[action_id={action}]"),
            span: None,
            hint: Some(
                "give each affordance its own action id, or make eligibility mutually \
                 exclusive with a non-stochastic conjunct so at most one candidate is ever \
                 eligible at once"
                    .to_owned(),
            ),
            details: None,
        });
    }
}

/// BL-39 / sk-j8e.3 — leftover duplicate offers. Two `affordance_template`
/// rows that share `niche_id` + `outcome_template_id` *and* identical
/// eligibility (`predicate_source` + `predicate_bool`) used to validate
/// clean; runtime then silently picked one via the `template_id` tie-break
/// (the crypt-fight range-0 leftover of `srd/attack/melee/affordance`).
/// `MF_VAL_AFFORDANCE_SELECTION_AMBIGUITY` groups by resolved action and
/// skips move/sense outcomes, so this identical-offer shape was invisible
/// there. Distinct predicates on the same niche+outcome (the core
/// traversal-gate catalog) are alternative gates, not identical offers,
/// and stay quiet. Range is *not* part of the key: a touch-range leftover
/// of a reach-bearing twin is the bug this catches.
///
/// Severity is **Warning**, matching the ADR-0021 D-2 Postscript / MF-VAL-243
/// ratchet: a new Fatal lint must not redden an in-flight corpus.
pub const MF_VAL_DUPLICATE_NICHE_OUTCOME_TEMPLATE: &str = "MF-VAL-327";

/// Flag `(niche_id, outcome_template_id, predicate)` groups carrying >=2
/// `affordance_template` rows. Empty `niche_id` or `outcome_template_id` is
/// skipped (those rows are not offers).
pub fn validate_duplicate_niche_outcome_templates(
    manifest: &Manifest,
    diags: &mut Vec<Diagnostic>,
) {
    // (niche, outcome, predicate_source, predicate_bool) -> template_ids.
    // BTreeMap → stable diagnostic order.
    let mut groups: BTreeMap<(&str, &str, &str, bool), Vec<&str>> = BTreeMap::new();
    for t in &manifest.affordance_templates {
        let niche = t.niche_id.trim();
        let outcome = t.outcome_template_id.trim();
        if niche.is_empty() || outcome.is_empty() {
            continue;
        }
        groups
            .entry((niche, outcome, t.predicate_source.trim(), t.predicate_bool))
            .or_default()
            .push(t.template_id.as_str());
    }

    for ((niche, outcome, _, _), mut templates) in groups {
        if templates.len() < 2 {
            continue;
        }
        templates.sort_unstable();
        diags.push(Diagnostic {
            code: MF_VAL_DUPLICATE_NICHE_OUTCOME_TEMPLATE.to_owned(),
            severity: Severity::Warning,
            message: format!(
                "{} affordance_templates share niche '{niche}' and outcome '{outcome}' \
                 with identical eligibility ({}). The template_id tie-break — not author \
                 intent — silently picks which offer fires.",
                templates.len(),
                templates.join(", "),
            ),
            path: format!("affordance_templates[niche_id={niche},outcome_template_id={outcome}]"),
            span: None,
            hint: Some(
                "delete the leftover duplicate, or give each template a distinct outcome \
                 or eligibility predicate so they are not interchangeable"
                    .to_owned(),
            ),
            details: None,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use spellkave_schema::{AffordanceTemplateRow, OutcomeTemplateRow};

    fn outcome(id: &str, action: &str) -> OutcomeTemplateRow {
        OutcomeTemplateRow {
            outcome_template_id: id.to_owned(),
            bundle_id: "test".to_owned(),
            display_name: id.to_owned(),
            outcome_bool: true,
            outcome_expression_source: String::new(),
            manipulation_action_id: action.to_owned(),
            communication_speech_act_id: String::new(),
            movement_descriptor: String::new(),
            sense_query_kind: String::new(),
            sense_visibility: spellkave_schema::VisibilityClass::Local,
            consequence_template_id: String::new(),
            discovery_validated: false,
            description: String::new(),
        }
    }

    fn affordance(template_id: &str, niche_id: &str, outcome_id: &str) -> AffordanceTemplateRow {
        AffordanceTemplateRow {
            template_id: template_id.to_owned(),
            bundle_id: "test".to_owned(),
            niche_id: niche_id.to_owned(),
            capability_id: String::new(),
            display_name: template_id.to_owned(),
            predicate_bool: true,
            predicate_source: String::new(),
            accepts_global_storm: false,
            locality_class: String::new(),
            reads_occupancy: false,
            description: String::new(),
            stratification_ok: true,
            outcome_template_id: outcome_id.to_owned(),
            range_m: 0.0,
            spends_channel: None,
            recharges_on: None,
            composes: None,

            movement_cost_expression_source: String::new(),
        }
    }

    fn run(manifest: Manifest) -> Vec<String> {
        let mut diags = Vec::new();
        validate_affordance_selection_ambiguity(&manifest, &mut diags);
        diags.into_iter().map(|d| d.code).collect()
    }

    /// Two affordances on one niche resolving to ONE action → a Warning naming both.
    #[test]
    fn overlapping_pair_warns() {
        let manifest = Manifest {
            outcome_templates: vec![outcome("o/a", "strike"), outcome("o/b", "strike")],
            affordance_templates: vec![
                affordance("aff/a", "n/combat", "o/a"),
                affordance("aff/b", "n/combat", "o/b"),
            ],
            ..Manifest::default()
        };
        assert_eq!(run(manifest), vec![MF_VAL_AFFORDANCE_SELECTION_AMBIGUITY]);
    }

    /// A single eligible affordance for an action at a niche → no diagnostic.
    #[test]
    fn single_candidate_does_not_warn() {
        let manifest = Manifest {
            outcome_templates: vec![outcome("o/a", "strike")],
            affordance_templates: vec![affordance("aff/a", "n/combat", "o/a")],
            ..Manifest::default()
        };
        assert!(run(manifest).is_empty());
    }

    /// Same niche but DIFFERENT resolved actions → not a selection collision.
    #[test]
    fn same_niche_distinct_actions_does_not_warn() {
        let manifest = Manifest {
            outcome_templates: vec![outcome("o/a", "play_cards"), outcome("o/b", "haggle")],
            affordance_templates: vec![
                affordance("aff/a", "n/table", "o/a"),
                affordance("aff/b", "n/table", "o/b"),
            ],
            ..Manifest::default()
        };
        assert!(run(manifest).is_empty());
    }

    /// Move/sense affordances (outcome carries no manipulation_action_id) are not
    /// selection-priority candidates and never warn, even sharing a niche+outcome.
    #[test]
    fn move_affordances_without_action_do_not_warn() {
        let manifest = Manifest {
            outcome_templates: vec![outcome("core/move_outcome", "")],
            affordance_templates: vec![
                affordance(
                    "core/traversal/always",
                    "core/traversable_edge",
                    "core/move_outcome",
                ),
                affordance(
                    "core/traversal/trace",
                    "core/traversable_edge",
                    "core/move_outcome",
                ),
            ],
            ..Manifest::default()
        };
        assert!(run(manifest).is_empty());
    }
}

#[cfg(test)]
mod stochastic_overlap_tests {
    use super::*;
    use spellkave_schema::{AffordanceTemplateRow, OutcomeTemplateRow};

    fn outcome(id: &str, action: &str) -> OutcomeTemplateRow {
        OutcomeTemplateRow {
            outcome_template_id: id.to_owned(),
            bundle_id: "test".to_owned(),
            display_name: id.to_owned(),
            outcome_bool: true,
            outcome_expression_source: String::new(),
            manipulation_action_id: action.to_owned(),
            communication_speech_act_id: String::new(),
            movement_descriptor: String::new(),
            sense_query_kind: String::new(),
            sense_visibility: spellkave_schema::VisibilityClass::Local,
            consequence_template_id: String::new(),
            discovery_validated: false,
            description: String::new(),
        }
    }

    /// Distinct niches, unlike the sibling `affordance_selection_ambiguity` tests
    /// above — the whole point of this lint is catching a cross-niche overlap.
    fn affordance(
        template_id: &str,
        niche_id: &str,
        outcome_id: &str,
        predicate_source: &str,
    ) -> AffordanceTemplateRow {
        AffordanceTemplateRow {
            template_id: template_id.to_owned(),
            bundle_id: "test".to_owned(),
            niche_id: niche_id.to_owned(),
            capability_id: String::new(),
            display_name: template_id.to_owned(),
            predicate_bool: true,
            predicate_source: predicate_source.to_owned(),
            accepts_global_storm: false,
            locality_class: String::new(),
            reads_occupancy: false,
            description: String::new(),
            stratification_ok: true,
            outcome_template_id: outcome_id.to_owned(),
            range_m: 0.0,
            spends_channel: None,
            recharges_on: None,
            composes: None,

            movement_cost_expression_source: String::new(),
        }
    }

    const STOCHASTIC_A: &str = "d20 + 5 >= target.stat('srd/ac')";
    const STOCHASTIC_B: &str = "d20 >= target.stat('srd/ac')";
    const STATIC_PREDICATE: &str = "target.stat('srd/ac') >= 1";

    fn run(manifest: Manifest) -> Vec<String> {
        let mut diags = Vec::new();
        validate_stochastic_affordance_overlap(&manifest, &mut diags);
        diags.into_iter().map(|d| d.code).collect()
    }

    /// (a) Two DIFFERENT niches, same resolved action, EACH predicate
    /// independently stochastic → warns. This is the #1556 shape:
    /// `MF_VAL_AFFORDANCE_SELECTION_AMBIGUITY` does not catch it (different
    /// niches) but the act-time fan-out still evaluates both with fresh draws.
    #[test]
    fn two_stochastic_same_action_different_niches_warns() {
        let manifest = Manifest {
            outcome_templates: vec![
                outcome("o/reaction/hit", "srd/attack/melee"),
                outcome("o/melee/hit", "srd/attack/melee"),
            ],
            affordance_templates: vec![
                affordance(
                    "phandelver/hero/reactive_strike/affordance",
                    "capability:phandelver/hero/reactive_strike",
                    "o/reaction/hit",
                    STOCHASTIC_A,
                ),
                affordance(
                    "srd/attack/melee/affordance",
                    "srd/niche/strikeable_target",
                    "o/melee/hit",
                    STOCHASTIC_B,
                ),
            ],
            ..Manifest::default()
        };
        assert_eq!(run(manifest), vec![MF_VAL_STOCHASTIC_AFFORDANCE_OVERLAP]);
    }

    /// (b) Same action, only ONE of the two predicates is stochastic (the other
    /// is a static comparison with no draw) → no warning; there is only ever one
    /// independent roll for the act, exactly as authored.
    #[test]
    fn same_action_single_stochastic_predicate_does_not_warn() {
        let manifest = Manifest {
            outcome_templates: vec![
                outcome("o/a", "srd/attack/melee"),
                outcome("o/b", "srd/attack/melee"),
            ],
            affordance_templates: vec![
                affordance("aff/a", "niche/a", "o/a", STOCHASTIC_A),
                affordance("aff/b", "niche/b", "o/b", STATIC_PREDICATE),
            ],
            ..Manifest::default()
        };
        assert!(run(manifest).is_empty());
    }

    /// (c) Two stochastic predicates but resolving DIFFERENT actions → not an
    /// overlap; the act-time fan-out never evaluates both for one requested
    /// action, so there is no independent-draws advantage.
    #[test]
    fn two_stochastic_different_actions_does_not_warn() {
        let manifest = Manifest {
            outcome_templates: vec![
                outcome("o/a", "srd/attack/melee"),
                outcome("o/b", "srd/attack/ranged"),
            ],
            affordance_templates: vec![
                affordance("aff/a", "niche/a", "o/a", STOCHASTIC_A),
                affordance("aff/b", "niche/b", "o/b", STOCHASTIC_B),
            ],
            ..Manifest::default()
        };
        assert!(run(manifest).is_empty());
    }

    /// (d) A single affordance resolving an action (no candidate to overlap
    /// with) → no warning, regardless of its predicate's stochasticity.
    #[test]
    fn single_affordance_does_not_warn() {
        let manifest = Manifest {
            outcome_templates: vec![outcome("o/a", "srd/attack/melee")],
            affordance_templates: vec![affordance("aff/a", "niche/a", "o/a", STOCHASTIC_A)],
            ..Manifest::default()
        };
        assert!(run(manifest).is_empty());
    }
}

#[cfg(test)]
mod duplicate_niche_outcome_tests {
    use super::*;
    use spellkave_schema::AffordanceTemplateRow;

    fn affordance(
        template_id: &str,
        niche_id: &str,
        outcome_id: &str,
        predicate_source: &str,
        range_m: f32,
    ) -> AffordanceTemplateRow {
        AffordanceTemplateRow {
            template_id: template_id.to_owned(),
            bundle_id: "test".to_owned(),
            niche_id: niche_id.to_owned(),
            capability_id: String::new(),
            display_name: template_id.to_owned(),
            predicate_bool: true,
            predicate_source: predicate_source.to_owned(),
            accepts_global_storm: false,
            locality_class: String::new(),
            reads_occupancy: false,
            description: String::new(),
            stratification_ok: true,
            outcome_template_id: outcome_id.to_owned(),
            range_m,
            spends_channel: None,
            recharges_on: None,
            composes: None,
            movement_cost_expression_source: String::new(),
        }
    }

    fn run(manifest: Manifest) -> Vec<String> {
        let mut diags = Vec::new();
        validate_duplicate_niche_outcome_templates(&manifest, &mut diags);
        diags.into_iter().map(|d| d.code).collect()
    }

    /// Crypt-fight leftover shape: same niche + same outcome + same
    /// eligibility, different range_m (and template_id). Must warn even when
    /// the outcome has no manipulation_action_id (doors / move).
    #[test]
    fn identical_niche_outcome_different_range_warns() {
        let manifest = Manifest {
            affordance_templates: vec![
                affordance("dup/door_a", "dup/door", "dup/open", "", 1.5),
                affordance("dup/door_b", "dup/door", "dup/open", "", 0.0),
            ],
            ..Manifest::default()
        };
        assert_eq!(run(manifest), vec![MF_VAL_DUPLICATE_NICHE_OUTCOME_TEMPLATE]);
    }

    /// Core traversal-gate catalog: same niche + same outcome, DISTINCT
    /// predicates. Alternative gates, not identical offers.
    #[test]
    fn same_niche_outcome_distinct_predicates_does_not_warn() {
        let manifest = Manifest {
            affordance_templates: vec![
                affordance(
                    "core/traversal/always",
                    "core/traversable_edge",
                    "core/traversal/move_outcome",
                    "",
                    0.0,
                ),
                affordance(
                    "core/traversal/trace_present",
                    "core/traversable_edge",
                    "core/traversal/move_outcome",
                    "target.trace(\"scrape_marks\") == true",
                    0.0,
                ),
            ],
            ..Manifest::default()
        };
        assert!(run(manifest).is_empty());
    }

    #[test]
    fn single_template_does_not_warn() {
        let manifest = Manifest {
            affordance_templates: vec![affordance("dup/door_a", "dup/door", "dup/open", "", 0.0)],
            ..Manifest::default()
        };
        assert!(run(manifest).is_empty());
    }

    #[test]
    fn empty_niche_or_outcome_is_skipped() {
        let manifest = Manifest {
            affordance_templates: vec![
                affordance("dup/a", "", "dup/open", "", 0.0),
                affordance("dup/b", "", "dup/open", "", 0.0),
                affordance("dup/c", "dup/door", "", "", 0.0),
                affordance("dup/d", "dup/door", "", "", 0.0),
            ],
            ..Manifest::default()
        };
        assert!(run(manifest).is_empty());
    }
}
