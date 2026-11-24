//! RED-first acceptance for ADR-0045: full author-facing arithmetic on Int
//! expressions and the new `cond` op (D-3).
//!
//! Dice are the built-in `dN`/`XdN` shortcut (server arithmetic, no table);
//! authored roll TABLES keep their own `roll_table(...)` surface and are not in
//! scope here. Against `main` these all fail at parse time (`+`/`*`/`/` are
//! unlexed, unary minus and grouping are unparsed, and `cond` is not an op).
//! This file pins the surface the worked-example spells require; it goes GREEN
//! once the grammar + `Op::Cond` land.

use spellkave_dsl::{
    evaluate_int_expression, parse_int_expression, DeterministicRng, EvaluationContext,
    EvaluationLimits,
};

/// Parse `src` as an Int expression and evaluate it with a deterministic RNG
/// whose successive `next_u32()` values are `draws` (a `dN` roll yields
/// `draw % N + 1`).
fn eval(src: &str, draws: Vec<u32>) -> i64 {
    let expr = parse_int_expression(src).unwrap_or_else(|e| {
        panic!("parse `{src}` failed: {:?}", e.diagnostics);
    });
    let facts = EvaluationContext::default();
    let mut rng = DeterministicRng::new(draws);
    evaluate_int_expression(&expr, &facts, &mut rng, EvaluationLimits::default())
        .unwrap_or_else(|e| panic!("eval `{src}` failed: {e:?}"))
}

#[test]
fn full_arithmetic_parses_and_evaluates() {
    assert_eq!(eval("1 + 2", vec![]), 3);
    assert_eq!(eval("10 / 2 - 1", vec![]), 4);
    // `*` and `/` bind tighter than `+`/`-`.
    assert_eq!(eval("1 + 2 * 3", vec![]), 7);
    assert_eq!(eval("2 * 3 + 1", vec![]), 7);
    // Grouping overrides precedence.
    assert_eq!(eval("(1 + 2) * 3", vec![]), 9);
    // `+`/`-` chain left-associatively.
    assert_eq!(eval("10 - 3 - 2", vec![]), 5);
    // Unary minus.
    assert_eq!(eval("-3", vec![]), -3);
    assert_eq!(eval("-(1 + 2)", vec![]), -3);
}

#[test]
fn dice_shortcut_composes_with_arithmetic() {
    // Built-in dice are the bare `dN`/`XdN` atom — no table, no `roll()` wrapper.
    assert_eq!(eval("d6", vec![3]), 4); // 3 % 6 + 1
    assert_eq!(eval("2d4", vec![0, 3]), 5); // (0%4+1) + (3%4+1) = 1 + 4
                                            // Magic Missile damage as a negative HP delta: -(1d4) - 1.
    assert_eq!(eval("-d4 - 1", vec![2]), -4); // -(2%4+1) - 1 = -3 - 1
}

#[test]
fn cond_selects_one_branch_lazily() {
    assert_eq!(eval("cond(1 > 0, 10, 20)", vec![]), 10);
    assert_eq!(eval("cond(1 < 0, 10, 20)", vec![]), 20);
    // Laziness (the K-5 distinguisher): the untaken branch is never evaluated,
    // so a division-by-zero in `else` does not error when `then` is selected.
    assert_eq!(eval("cond(1 > 0, 5, 1 / 0)", vec![]), 5);
    // Save-for-half over a SINGLE bound roll — the composition `cond` exists to
    // express: one draw, halved on the taken branch, never re-rolled.
    assert_eq!(eval("let r = d6; cond(1 > 0, r / 2, r)", vec![5]), 3); // r=6 -> 6/2
}

/// Guard for issue #1284: the DSL `Div` op evaluates Rust integer `/`, which
/// TRUNCATES toward zero, whereas the SRD ability modifier is `floor((score-10)/2)`.
/// They diverge for odd ability scores BELOW 10 (negative-odd numerator): STR 9 ->
/// trunc 0 vs floor -1. The canon-warden gate (#1284) rejected a new `Floor`
/// kernel op (K-5: floor decomposes), so the SRD read sites compose the exact
/// floor instead: `(a)/2 + cond(a - (a/2)*2 < 0, -1, 0)`, a = score-10. This test
/// pins that compose form against regression — it is the test the op would have
/// needed, repurposed to guard the authored expression.
#[test]
fn floor_ability_modifier_composes_exactly_for_negative_odd_scores() {
    // The exact compose form the SRD fixtures author (a = score - 10).
    let floor_mod = |score: i64| {
        format!("({score} - 10) / 2 + cond(({score} - 10) - (({score} - 10) / 2) * 2 < 0, -1, 0)")
    };
    // The naive (buggy) bare `/` form, for contrast.
    let trunc_mod = |score: i64| format!("({score} - 10) / 2");

    // (score, true SRD floor modifier)
    let cases = [
        (1, -5), // a = -9: trunc -4, floor -5
        (3, -4), // a = -7: trunc -3, floor -4
        (5, -3), // a = -5: trunc -2, floor -3
        (7, -2), // a = -3: trunc -1, floor -2
        (9, -1), // a = -1: trunc  0, floor -1  (the #1284 headline case)
        (8, -1), // a = -2 (even): trunc == floor == -1
        (10, 0), // a = 0
        (12, 1), // a = 2
        (15, 2), // a = 5: trunc == floor == 2 (odd, but a >= 0)
        (17, 3), // a = 7
        (20, 5), // a = 10
    ];
    for (score, expected_floor) in cases {
        assert_eq!(
            eval(&floor_mod(score), vec![]),
            expected_floor,
            "compose form must yield SRD floor for score {score}",
        );
    }

    // The divergence is real and exactly +1 for negative-odd scores: the bare `/`
    // over-reports the modifier (the latent bug #1284 fixes).
    for (score, floor) in [(1, -5), (3, -4), (5, -3), (7, -2), (9, -1)] {
        assert_eq!(
            eval(&trunc_mod(score), vec![]),
            floor + 1,
            "bare / truncates for {score}"
        );
    }
    // ...and agrees with floor for even or >= 10 scores (the common case).
    for score in [8, 10, 12, 14, 15, 17, 20] {
        assert_eq!(
            eval(&trunc_mod(score), vec![]),
            eval(&floor_mod(score), vec![]),
            "bare / == floor for non-(negative-odd) score {score}",
        );
    }
}
