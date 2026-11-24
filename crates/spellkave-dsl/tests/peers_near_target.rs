//! K-A Fact 2: `peers_near_target(prefix,
//! radius_mm, excluding_condition)` — a K-5 leaf counting actors other than
//! Actor and Target within `radius_mm` of Target that share a `prefix`-tag
//! with Actor and do not carry the excluding condition.
//!
//! This is the RED-first fixture the design doc's "K-5 evidence" row names:
//! before `Op::PeersNearTarget` existed, `parse_predicate`/`parse_int_expression`
//! rejected this call outright (`MF-DSL-EXPECTED-*` — no such function keyword).
//! These tests exercise the grammar (3-arg and optional-2-arg forms), the
//! fail-closed missing-fact behaviour (never a silent zero, distinct from
//! `ImpliedByCount`'s empty-default-zero), and exact-triple fact resolution
//! through the SAME public entry points production callers use.

use spellkave_dsl::{
    evaluate_int_expression, evaluate_predicate_expression, parse_int_expression, parse_predicate,
    DeterministicRng, EvaluationContext, EvaluationError, EvaluationLimits, Op,
    PeersNearTargetFact, SubjectRef,
};

fn no_draws() -> DeterministicRng {
    DeterministicRng::new(vec![])
}

#[test]
fn peers_near_target_parses_three_args_into_the_typed_op() {
    let expr = parse_int_expression(
        "peers_near_target('srd.faction.', 1524, 'srd/condition/incapacitated')",
    )
    .expect("peers_near_target should parse as an int expression");
    let root = expr.node(expr.root_node_id).expect("root node exists");
    match &root.op {
        Op::PeersNearTarget(op) => {
            assert_eq!(op.prefix, "srd.faction.");
            assert_eq!(op.radius_mm, 1524);
            assert_eq!(
                op.excluding_condition_definition_id.as_deref(),
                Some("srd/condition/incapacitated")
            );
        }
        other => panic!("expected Op::PeersNearTarget, got {other:?}"),
    }
}

#[test]
fn peers_near_target_excluding_condition_argument_is_optional() {
    let expr = parse_int_expression("peers_near_target('srd.faction.', 1524)")
        .expect("two-arg peers_near_target should parse");
    let root = expr.node(expr.root_node_id).expect("root node exists");
    match &root.op {
        Op::PeersNearTarget(op) => {
            assert_eq!(op.prefix, "srd.faction.");
            assert_eq!(op.radius_mm, 1524);
            assert_eq!(op.excluding_condition_definition_id, None);
        }
        other => panic!("expected Op::PeersNearTarget, got {other:?}"),
    }
}

/// The evaluator NEVER touches a database or computes geometry — a missing
/// pre-populated fact is an evaluation error, never a silent zero (design
/// doc: "A missing pre-populated fact is an evaluation error, never a
/// silent zero" — the sharpest possible contrast with `ImpliedByCount`,
/// whose empty context legitimately defaults to 0).
#[test]
fn peers_near_target_with_no_prepopulated_fact_is_an_evaluation_error() {
    let expr = parse_int_expression(
        "peers_near_target('srd.faction.', 1524, 'srd/condition/incapacitated')",
    )
    .expect("parses");
    let mut rng = no_draws();

    let result = evaluate_int_expression(
        &expr,
        &EvaluationContext::default(),
        &mut rng,
        EvaluationLimits::default(),
    );

    match result {
        Err(EvaluationError::UnknownField { subject, .. }) => {
            assert_eq!(subject, SubjectRef::Target);
        }
        other => panic!(
            "expected EvaluationError::UnknownField, got {other:?} — a missing \
             pre-populated fact must fail loud, never resolve to 0"
        ),
    }
}

#[test]
fn peers_near_target_resolves_the_exact_matching_triple() {
    let expr = parse_predicate(
        "peers_near_target('srd.faction.', 1524, 'srd/condition/incapacitated') >= 1",
    )
    .expect("parses as a predicate via comparison");
    let facts = EvaluationContext {
        peers_near_target: vec![PeersNearTargetFact {
            prefix: "srd.faction.".into(),
            radius_mm: 1524,
            excluding_condition_definition_id: Some("srd/condition/incapacitated".into()),
            count: 2,
        }],
        ..Default::default()
    };
    let mut rng = no_draws();

    assert_eq!(
        evaluate_predicate_expression(&expr, &facts, &mut rng, EvaluationLimits::default()),
        Ok(true)
    );
}

/// A fact for a DIFFERENT triple (radius, prefix, or exclusion differ) must
/// not silently satisfy a call it does not literally match — pinning that
/// the evaluator matches all three fields, not just the prefix.
#[test]
fn peers_near_target_does_not_match_a_fact_for_a_different_triple() {
    let expr = parse_int_expression(
        "peers_near_target('srd.faction.', 1524, 'srd/condition/incapacitated')",
    )
    .expect("parses");
    let facts = EvaluationContext {
        peers_near_target: vec![PeersNearTargetFact {
            prefix: "srd.faction.".into(),
            radius_mm: 3048, // different radius_mm than the call
            excluding_condition_definition_id: Some("srd/condition/incapacitated".into()),
            count: 5,
        }],
        ..Default::default()
    };
    let mut rng = no_draws();

    let result = evaluate_int_expression(&expr, &facts, &mut rng, EvaluationLimits::default());
    assert!(
        matches!(result, Err(EvaluationError::UnknownField { .. })),
        "a fact for a different radius_mm must not satisfy this call: {result:?}"
    );
}

#[test]
fn peers_near_target_none_excluding_condition_matches_a_none_fact_only() {
    let expr = parse_int_expression("peers_near_target('srd.faction.', 1524)").expect("parses");
    let facts_with_exclusion = EvaluationContext {
        peers_near_target: vec![PeersNearTargetFact {
            prefix: "srd.faction.".into(),
            radius_mm: 1524,
            excluding_condition_definition_id: Some("srd/condition/incapacitated".into()),
            count: 9,
        }],
        ..Default::default()
    };
    let mut rng = no_draws();
    let result = evaluate_int_expression(
        &expr,
        &facts_with_exclusion,
        &mut rng,
        EvaluationLimits::default(),
    );
    assert!(
        matches!(result, Err(EvaluationError::UnknownField { .. })),
        "the no-exclusion call must not match a fact scoped to an exclusion: {result:?}"
    );

    let facts_without_exclusion = EvaluationContext {
        peers_near_target: vec![PeersNearTargetFact {
            prefix: "srd.faction.".into(),
            radius_mm: 1524,
            excluding_condition_definition_id: None,
            count: 3,
        }],
        ..Default::default()
    };
    assert_eq!(
        evaluate_int_expression(
            &expr,
            &facts_without_exclusion,
            &mut rng,
            EvaluationLimits::default()
        ),
        Ok(3)
    );
}
