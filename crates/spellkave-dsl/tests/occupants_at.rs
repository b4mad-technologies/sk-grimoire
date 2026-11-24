//! ADR-0134 D-3: `occupants_at('<scope>', '<excluding_condition>')` — a K-5
//! leaf counting actors other than Actor that hold an `actor_spatial_location`
//! row on the scoped place node and do not carry the excluding condition.
//!
//! This is the RED-first fixture the ADR's K-5 row names. Before
//! `Op::OccupantsAt` existed, `parse_predicate` rejected the call outright
//! (`MF-DSL-EXPECTED-*` — no such function keyword) and no
//! `EvaluationContext` field could carry the count, so this file did not
//! compile. The driving case is the elven-mini cellar stalemate: a dead rat
//! on the destination cell must not block the SRD step template's
//! `occupants_at('to_node', 'srd/condition/dead') == 0` clause, while a
//! living occupant must. The evaluator never reads the database: the kernel
//! pre-populates one fact per `(scope, excluding_condition)` pair the
//! expression literally references, and a scope the current path did not
//! populate is an evaluation error, never a silent zero (the
//! `PeersNearTarget` rule, ADR-0105 D-2).

use spellkave_dsl::{
    evaluate_int_expression, evaluate_predicate_expression, parse_int_expression, parse_predicate,
    DeterministicRng, EvaluationContext, EvaluationError, EvaluationLimits, OccupantsAtFact, Op,
    SubjectRef,
};

const STEP_PREDICATE: &str = "occupants_at('to_node', 'srd/condition/dead') == 0";

fn no_draws() -> DeterministicRng {
    DeterministicRng::new(vec![])
}

fn fact(scope: &str, excluding: Option<&str>, count: i64) -> OccupantsAtFact {
    OccupantsAtFact {
        scope: scope.into(),
        excluding_condition_definition_id: excluding.map(str::to_owned),
        count,
    }
}

/// One occupant stands on the destination. The kernel populates both pairs
/// a predicate could name: the unfiltered count says "one body is there",
/// the `srd/condition/dead`-filtered count says how many of them are alive.
fn destination_with_one_occupant(occupant_is_dead: bool) -> EvaluationContext {
    let living = if occupant_is_dead { 0 } else { 1 };
    EvaluationContext {
        occupants_at: vec![
            fact("to_node", None, 1),
            fact("to_node", Some("srd/condition/dead"), living),
        ],
        ..Default::default()
    }
}

#[test]
fn k5_adr0134_occupants_at_parses_scope_and_optional_condition() {
    let expr = parse_int_expression("occupants_at('to_node', 'srd/condition/dead')")
        .expect("occupants_at should parse as an int expression");
    let root = expr.node(expr.root_node_id).expect("root node exists");
    match &root.op {
        Op::OccupantsAt(op) => {
            assert_eq!(op.scope, "to_node");
            assert_eq!(
                op.excluding_condition_definition_id.as_deref(),
                Some("srd/condition/dead")
            );
        }
        other => panic!("expected Op::OccupantsAt, got {other:?}"),
    }

    let expr = parse_int_expression("occupants_at('from_node')")
        .expect("one-arg occupants_at should parse");
    let root = expr.node(expr.root_node_id).expect("root node exists");
    match &root.op {
        Op::OccupantsAt(op) => {
            assert_eq!(op.scope, "from_node");
            assert_eq!(op.excluding_condition_definition_id, None);
        }
        other => panic!("expected Op::OccupantsAt, got {other:?}"),
    }
}

/// The ADR-0134 D-3 falsifying fixture: the SRD step clause passes over a
/// dead occupant and fails over a living one. Without the leaf the clause
/// cannot be authored at all, and the kernel veto blocked both cases alike.
#[test]
fn k5_adr0134_step_over_a_dead_occupant_passes_and_over_a_living_one_fails() {
    let expr = parse_predicate(STEP_PREDICATE).expect("the SRD step clause parses");
    let mut rng = no_draws();

    assert_eq!(
        evaluate_predicate_expression(
            &expr,
            &destination_with_one_occupant(true),
            &mut rng,
            EvaluationLimits::default()
        ),
        Ok(true),
        "a corpse on the destination does not block the step"
    );
    assert_eq!(
        evaluate_predicate_expression(
            &expr,
            &destination_with_one_occupant(false),
            &mut rng,
            EvaluationLimits::default()
        ),
        Ok(false),
        "a living occupant on the destination blocks the step"
    );
}

/// The unfiltered form counts every occupant, dead or alive — the second
/// argument is what makes "corpse is an object" cost the author one string.
#[test]
fn k5_adr0134_occupants_at_without_a_condition_counts_every_occupant() {
    let expr = parse_int_expression("occupants_at('to_node')").expect("parses");
    let mut rng = no_draws();
    assert_eq!(
        evaluate_int_expression(
            &expr,
            &destination_with_one_occupant(true),
            &mut rng,
            EvaluationLimits::default()
        ),
        Ok(1)
    );
}

/// A scope the current path did not populate is an evaluation error, never
/// a silent zero: a `from_node`-only fact pool must not satisfy a `to_node`
/// read, and an empty pool must not either.
#[test]
fn k5_adr0134_occupants_at_unpopulated_scope_is_an_evaluation_error() {
    let expr = parse_predicate(STEP_PREDICATE).expect("parses");
    let mut rng = no_draws();

    for facts in [
        EvaluationContext::default(),
        EvaluationContext {
            occupants_at: vec![fact("from_node", Some("srd/condition/dead"), 0)],
            ..Default::default()
        },
        EvaluationContext {
            occupants_at: vec![fact("to_node", None, 0)],
            ..Default::default()
        },
    ] {
        let result =
            evaluate_predicate_expression(&expr, &facts, &mut rng, EvaluationLimits::default());
        match result {
            Err(EvaluationError::UnknownField { subject, field_id }) => {
                assert_eq!(subject, SubjectRef::Target);
                assert!(
                    field_id.starts_with("occupants_at:to_node:"),
                    "error names the unpopulated pair: {field_id}"
                );
            }
            other => panic!(
                "expected EvaluationError::UnknownField, got {other:?} — a missing \
                 pre-populated pair must fail loud, never resolve to 0"
            ),
        }
    }
}
