//! Shared evaluation of `entity_definitions.slot_compatibility_predicate` (ADR-0118).
//! Accepts an empty string, `true`, `false` or `slot_id == "<value>"`.
//! Unknown syntax fails closed.

/// Outcome of evaluating a `slot_compatibility_predicate` against a target
/// `slot_id`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompatibilityPredicateVerdict {
    /// The predicate accepts the slot.
    Matches,
    /// The predicate parsed but rejects the slot.
    DoesNotMatch,
    /// The predicate is not one of the supported forms.
    UnknownSyntax,
}

/// Evaluate an authored `slot_compatibility_predicate` against a target
/// `slot_id`.
///
/// Supported forms:
/// - `""` or `"true"` -> compatible with any slot.
/// - `"false"` -> incompatible with all slots.
/// - `slot_id == "<value>"` -> compatible only when slot_id equals value.
///
/// Any other input is `UnknownSyntax` — fail-closed, mirroring the runtime's
/// own posture (an unrecognized predicate rejects the equip rather than
/// silently admitting it).
pub fn evaluate_compatibility_predicate(
    predicate: &str,
    slot_id: &str,
) -> CompatibilityPredicateVerdict {
    let p = predicate.trim();
    if p.is_empty() || p == "true" {
        return CompatibilityPredicateVerdict::Matches;
    }
    if p == "false" {
        return CompatibilityPredicateVerdict::DoesNotMatch;
    }
    if let Some(rest) = p.strip_prefix("slot_id == \"") {
        if let Some(expected) = rest.strip_suffix('"') {
            if slot_id == expected {
                return CompatibilityPredicateVerdict::Matches;
            }
            return CompatibilityPredicateVerdict::DoesNotMatch;
        }
    }
    CompatibilityPredicateVerdict::UnknownSyntax
}

/// Human-readable reason a `slot_compatibility_predicate` rejects a
/// `slot_id`, or `None` when it matches (compatible). Shared text builder
/// for both static-validator diagnostic messages (MF-VAL-315, MF-VAL-326) —
/// each call site prefixes/suffixes it with its own row-identifying context.
pub fn incompatibility_reason(predicate: &str, slot_id: &str) -> Option<String> {
    match evaluate_compatibility_predicate(predicate, slot_id) {
        CompatibilityPredicateVerdict::Matches => None,
        CompatibilityPredicateVerdict::DoesNotMatch => {
            let trimmed = predicate.trim();
            if trimmed == "false" {
                Some(
                    "slot_compatibility_predicate is 'false' (incompatible with every slot)".into(),
                )
            } else {
                let expected = trimmed
                    .strip_prefix("slot_id == \"")
                    .and_then(|rest| rest.strip_suffix('"'))
                    .unwrap_or(trimmed);
                Some(format!(
                    "slot_compatibility_predicate requires slot_id == \"{expected}\", got \"{slot_id}\""
                ))
            }
        }
        CompatibilityPredicateVerdict::UnknownSyntax => Some(format!(
            "slot_compatibility_predicate '{predicate}' has unrecognized syntax"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_string_is_compatible() {
        assert_eq!(
            evaluate_compatibility_predicate("", "core/head"),
            CompatibilityPredicateVerdict::Matches
        );
        assert_eq!(incompatibility_reason("", "core/head"), None);
    }

    #[test]
    fn true_is_compatible() {
        assert_eq!(
            evaluate_compatibility_predicate("true", "core/head"),
            CompatibilityPredicateVerdict::Matches
        );
    }

    #[test]
    fn false_is_incompatible_with_everything() {
        assert_eq!(
            evaluate_compatibility_predicate("false", "core/head"),
            CompatibilityPredicateVerdict::DoesNotMatch
        );
        assert!(incompatibility_reason("false", "core/head").is_some());
    }

    #[test]
    fn slot_id_eq_matches_exact_and_rejects_others() {
        assert_eq!(
            evaluate_compatibility_predicate(r#"slot_id == "core/main_hand""#, "core/main_hand"),
            CompatibilityPredicateVerdict::Matches
        );
        assert_eq!(
            evaluate_compatibility_predicate(r#"slot_id == "core/main_hand""#, "core/off_hand"),
            CompatibilityPredicateVerdict::DoesNotMatch
        );
        assert!(
            incompatibility_reason(r#"slot_id == "core/main_hand""#, "core/off_hand").is_some()
        );
    }

    #[test]
    fn unknown_syntax_is_fail_closed() {
        assert_eq!(
            evaluate_compatibility_predicate("slot_tag:ring OR slot_tag:trinket", "core/ring_left"),
            CompatibilityPredicateVerdict::UnknownSyntax
        );
        assert!(
            incompatibility_reason("slot_tag:ring OR slot_tag:trinket", "core/ring_left").is_some()
        );
    }

    #[test]
    fn whitespace_is_trimmed() {
        assert_eq!(
            evaluate_compatibility_predicate("  true  ", "core/main_hand"),
            CompatibilityPredicateVerdict::Matches
        );
    }
}
