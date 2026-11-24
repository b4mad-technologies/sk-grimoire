//! Checks presentation placeholders in authored outcome descriptions.
//! The closed vocabulary names roles such as actor and target. The caller
//! substitutes visible names and keeps interpolation separate from rule evaluation.

use crate::{Diagnostic, Severity};
use spellkave_schema::Manifest;

/// `outcome_templates[].description` references an unknown `{...}` placeholder.
pub const MF_VAL_OBSERVATION_PLACEHOLDER: &str = "MF-VAL-294";
/// `outcome_templates[].description` exceeds [`OBSERVATION_DESCRIPTION_MAX_CHARS`].
pub const MF_VAL_OBSERVATION_DESCRIPTION_LENGTH: &str = "MF-VAL-295";

/// The FROZEN closed set of presentation-layer placeholder tokens an authored
/// `outcome_template.description` may reference (braces included, matching the
/// authoring surface verbatim). See the module doc comment for why this set
/// does not grow by convention.
pub const OBSERVATION_PLACEHOLDER_TOKENS: &[&str] =
    &["{target_name}", "{actor_name}", "{place_name}"];

/// Bounded authoring length for `outcome_template.description` (chars, not
/// bytes) — generous for a sentence or two of prose, small enough to keep
/// this a flavour line, not an essay smuggled past the "presentation only"
/// contract (see [`crate`]'s widened description-class doc source in
/// `spellkave_schema::Manifest`).
pub const OBSERVATION_DESCRIPTION_MAX_CHARS: usize = 500;

/// Validate every `outcome_templates[].description`: length is bounded, and
/// every brace-delimited token inside is a member of
/// [`OBSERVATION_PLACEHOLDER_TOKENS`]. An empty description is always valid
/// (unauthored — the observation contract falls back to its honest gap).
pub fn validate_observation_placeholders(manifest: &Manifest, diags: &mut Vec<Diagnostic>) {
    for (i, row) in manifest.outcome_templates.iter().enumerate() {
        let description = row.description.as_str();
        if description.is_empty() {
            continue;
        }
        let path = format!("outcome_templates[{i}].description");
        let char_count = description.chars().count();
        if char_count > OBSERVATION_DESCRIPTION_MAX_CHARS {
            diags.push(Diagnostic {
                code: MF_VAL_OBSERVATION_DESCRIPTION_LENGTH.to_owned(),
                severity: Severity::Fatal,
                message: format!(
                    "outcome_template '{}': description is {char_count} chars, over the \
                     {OBSERVATION_DESCRIPTION_MAX_CHARS}-char bound",
                    row.outcome_template_id,
                ),
                path: path.clone(),
                span: None,
                hint: Some("keep authored observation prose to a sentence or two".to_owned()),
                details: None,
            });
        }
        for token in extract_placeholder_tokens(description) {
            if !OBSERVATION_PLACEHOLDER_TOKENS.contains(&token.as_str()) {
                diags.push(Diagnostic {
                    code: MF_VAL_OBSERVATION_PLACEHOLDER.to_owned(),
                    severity: Severity::Fatal,
                    message: format!(
                        "outcome_template '{}': description references unknown placeholder \
                         '{token}'. Valid placeholders: [{}].",
                        row.outcome_template_id,
                        OBSERVATION_PLACEHOLDER_TOKENS.join(", "),
                    ),
                    path: path.clone(),
                    span: None,
                    hint: Some(
                        "a state-dereferencing value (HP, a resource, a condition) is not \
                         expressible here — it must route through the DSL, never this \
                         presentation-layer vocabulary"
                            .to_owned(),
                    ),
                    details: Some(serde_json::json!({
                        "invalid_placeholder": token,
                        "valid_placeholders": OBSERVATION_PLACEHOLDER_TOKENS,
                    })),
                });
            }
        }
    }
}

/// Extracts every `{...}` substring from `text`, in order. An unclosed `{`
/// yields a token running to end-of-string rather than being silently
/// dropped, so a malformed placeholder still fails the closed-set check
/// loudly instead of passing through as literal, unvalidated text.
fn extract_placeholder_tokens(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find('{') {
        let after_open = &rest[start + 1..];
        match after_open.find('}') {
            Some(close) => {
                tokens.push(format!("{{{}}}", &after_open[..close]));
                rest = &after_open[close + 1..];
            }
            None => {
                tokens.push(format!("{{{after_open}"));
                break;
            }
        }
    }
    tokens
}

#[cfg(test)]
mod tests {
    use super::*;
    use spellkave_schema::{OutcomeTemplateRow, VisibilityClass};

    fn manifest_with(rows: Vec<OutcomeTemplateRow>) -> Manifest {
        Manifest {
            outcome_templates: rows,
            ..Manifest::default()
        }
    }

    fn row(outcome_template_id: &str, description: &str) -> OutcomeTemplateRow {
        OutcomeTemplateRow {
            outcome_template_id: outcome_template_id.into(),
            bundle_id: "core".into(),
            display_name: "test outcome".into(),
            outcome_bool: false,
            outcome_expression_source: String::new(),
            manipulation_action_id: String::new(),
            communication_speech_act_id: String::new(),
            movement_descriptor: String::new(),
            sense_query_kind: String::new(),
            sense_visibility: VisibilityClass::Local,
            consequence_template_id: String::new(),
            discovery_validated: false,
            description: description.into(),
        }
    }

    fn codes(diags: &[Diagnostic]) -> Vec<String> {
        diags.iter().map(|d| d.code.clone()).collect()
    }

    #[test]
    fn empty_description_is_always_valid() {
        let manifest = manifest_with(vec![row("core/open_gate/outcome", "")]);
        let mut diags = Vec::new();
        validate_observation_placeholders(&manifest, &mut diags);
        assert!(diags.is_empty());
    }

    #[test]
    fn plain_prose_with_no_placeholders_is_valid() {
        let manifest = manifest_with(vec![row(
            "core/open_gate/outcome",
            "The gate swings open with a groan.",
        )]);
        let mut diags = Vec::new();
        validate_observation_placeholders(&manifest, &mut diags);
        assert!(diags.is_empty());
    }

    #[test]
    fn every_frozen_token_is_individually_accepted() {
        for token in OBSERVATION_PLACEHOLDER_TOKENS {
            let manifest =
                manifest_with(vec![row("core/greet/outcome", &format!("{token} nods."))]);
            let mut diags = Vec::new();
            validate_observation_placeholders(&manifest, &mut diags);
            assert!(diags.is_empty(), "{token} must be accepted: {diags:?}");
        }
    }

    #[test]
    fn all_three_tokens_together_are_accepted() {
        let manifest = manifest_with(vec![row(
            "core/greet/outcome",
            "{actor_name} greets {target_name} at {place_name}.",
        )]);
        let mut diags = Vec::new();
        validate_observation_placeholders(&manifest, &mut diags);
        assert!(diags.is_empty(), "{diags:?}");
    }

    #[test]
    fn unknown_placeholder_is_rejected() {
        let manifest = manifest_with(vec![row(
            "core/attack/outcome",
            "You deal {damage_amount} damage to {target_name}.",
        )]);
        let mut diags = Vec::new();
        validate_observation_placeholders(&manifest, &mut diags);
        assert_eq!(
            codes(&diags),
            vec![MF_VAL_OBSERVATION_PLACEHOLDER.to_owned()]
        );
        assert!(diags[0].message.contains("{damage_amount}"));
    }

    /// The named example from the module doc comment: a state-dereferencing
    /// token (HP) must never quietly pass as a presentation-layer placeholder.
    #[test]
    fn state_dereferencing_token_is_rejected_not_silently_accepted() {
        let manifest = manifest_with(vec![row(
            "core/attack/outcome",
            "{target_name} drops to {target_hp} HP.",
        )]);
        let mut diags = Vec::new();
        validate_observation_placeholders(&manifest, &mut diags);
        assert!(codes(&diags).contains(&MF_VAL_OBSERVATION_PLACEHOLDER.to_owned()));
    }

    #[test]
    fn unclosed_brace_is_rejected_not_dropped() {
        let manifest = manifest_with(vec![row(
            "core/attack/outcome",
            "{target_name} flinches {oops",
        )]);
        let mut diags = Vec::new();
        validate_observation_placeholders(&manifest, &mut diags);
        assert!(codes(&diags).contains(&MF_VAL_OBSERVATION_PLACEHOLDER.to_owned()));
    }

    #[test]
    fn description_over_the_char_bound_is_rejected() {
        let too_long = "x".repeat(OBSERVATION_DESCRIPTION_MAX_CHARS + 1);
        let manifest = manifest_with(vec![row("core/attack/outcome", &too_long)]);
        let mut diags = Vec::new();
        validate_observation_placeholders(&manifest, &mut diags);
        assert!(codes(&diags).contains(&MF_VAL_OBSERVATION_DESCRIPTION_LENGTH.to_owned()));
    }

    #[test]
    fn description_at_the_char_bound_is_accepted() {
        let at_bound = "x".repeat(OBSERVATION_DESCRIPTION_MAX_CHARS);
        let manifest = manifest_with(vec![row("core/attack/outcome", &at_bound)]);
        let mut diags = Vec::new();
        validate_observation_placeholders(&manifest, &mut diags);
        assert!(diags.is_empty());
    }
}
