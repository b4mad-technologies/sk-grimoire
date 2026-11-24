//! ADR-0047 D-5: the DSL reference examples must parse. This test catches
//! drift between the examples and the grammar.

use spellkave_dsl::{dsl_reference, parse_int_expression, parse_outcome, parse_predicate};

/// Every `authoring: "expression"` operator example parses in at least one
/// expression context.
///
/// `expression_kind` is often `"predicate|outcome"`, and context-dependent
/// literals (`"10"`, `"'opened'"`) only parse in some contexts, so the guard is
/// "parses somewhere", not "parses as a specific kind" — that is enough to prove
/// the example is live DSL rather than a drifted string.
///
/// `authoring: "frame_write"` operators (ModifyResource, ApplyStatus, …) are
/// authored as structured consequence rows, NOT DSL text; their examples are
/// conceptual notation and are deliberately excluded — the classification lives
/// on the reference itself (`DslOperatorReference::authoring`), not in a
/// parallel hand-list here.
#[test]
fn every_reference_operator_example_parses() {
    let reference = dsl_reference();
    let mut failures = Vec::new();

    for op in &reference.operators {
        if op.authoring != "expression" {
            continue;
        }
        for example in &op.examples {
            let parses = parse_predicate(example).is_ok()
                || parse_outcome(example).is_ok()
                || parse_int_expression(example).is_ok();
            if !parses {
                failures.push(format!("{} :: {example:?}", op.operator));
            }
        }
    }

    assert!(
        failures.is_empty(),
        "expression-authored operator examples that no parser accepts — drifted from the grammar:\n{}",
        failures.join("\n"),
    );
}

/// The `authoring` classification is one of the two known kinds — guards against
/// a typo silently excluding an operator from the parse gate.
#[test]
fn every_operator_has_a_known_authoring_kind() {
    let bad: Vec<_> = dsl_reference()
        .operators
        .iter()
        .filter(|op| !matches!(op.authoring, "expression" | "frame_write"))
        .map(|op| format!("{}: {:?}", op.operator, op.authoring))
        .collect();
    assert!(
        bad.is_empty(),
        "operators with an unknown `authoring` kind: {bad:?}"
    );
}

/// Sanity: the reference is non-empty and every operator carries at least one
/// example, so the guard above can never pass vacuously.
#[test]
fn every_reference_operator_has_an_example() {
    let reference = dsl_reference();
    assert!(
        !reference.operators.is_empty(),
        "dsl_reference() has no operators"
    );
    let missing: Vec<_> = reference
        .operators
        .iter()
        .filter(|op| op.examples.is_empty())
        .map(|op| op.operator)
        .collect();
    assert!(
        missing.is_empty(),
        "operators with no documented example: {missing:?}"
    );
}
