//! ADR-0047 D-5 / sk-e0u.3: `tests/reference_examples.rs` proves every
//! `authoring: "expression"` operator example PARSES; this file proves each
//! one also EVALUATES to the result its `dsl_reference()` entry documents,
//! through the SAME public entry points production callers use
//! (`evaluate_predicate_expression` / `evaluate_int_expression`). Zero new
//! interpreter dispatch arms, no test-only DSL primitives — this composes
//! `parse_predicate`/`parse_outcome`/`parse_int_expression` and the shared
//! `EvaluationContext`/`DeterministicRng` machinery exactly as `eval.rs`'s
//! own unit tests do.
//!
//! `authoring: "frame_write"` operators stay excluded (no parser surface —
//! see `reference_examples.rs`).

use spellkave_dsl::{
    compile_text_expression_with_macros, dsl_reference, evaluate_expression_traced,
    evaluate_int_expression, evaluate_predicate_expression,
    evaluate_predicate_expression_with_report, parse_int_expression, parse_predicate, BindingFact,
    ChannelPoolFact, DeterministicRng, EvaluationContext, EvaluationLimits, EvaluationValue,
    EventRef, ItemDslSourceFact, OccupantsAtFact, ParticipantTagGroupCountFact,
    PeersNearTargetFact, RollTableEntryFact, SubjectBoolFact, SubjectIdFact, SubjectIntFact,
    SubjectRef, SubjectResourceBoundFact, SubjectTextFact,
};

/// How a documented `authoring: "expression"` example's result is proven,
/// through an EXISTING public eval entry point.
enum DslExampleExpect {
    /// Root type Bool: `parse_predicate` + `evaluate_predicate_expression`.
    Predicate {
        context: fn() -> EvaluationContext,
        rng: fn() -> DeterministicRng,
        expect: bool,
    },
    /// Root type Int: `parse_int_expression` + `evaluate_int_expression`.
    Int {
        context: fn() -> EvaluationContext,
        rng: fn() -> DeterministicRng,
        expect: i64,
    },
    /// Root type Text: no public entry point returns a bare `Text` value, so
    /// the harness composes the example into a synthetic
    /// `<example> == '<expect>'` PREDICATE — real parser grammar
    /// (`Op::Compare` over two Text operands) run through
    /// `evaluate_predicate_expression`, not a new primitive — and asserts it
    /// evaluates `true`. This proves the example resolves to exactly the
    /// documented text value using only existing surface.
    TextEquals {
        context: fn() -> EvaluationContext,
        rng: fn() -> DeterministicRng,
        expect: &'static str,
    },
    /// Root type Bool reached only through the GENERIC literal/read atom
    /// production (`BoolLiteral`) — `parse_predicate`'s top-level grammar
    /// requires a comparison, `And`/`Or`/`Not`, or a recognised
    /// subject-method call for a bare bool, so a raw `BoolLiteral` atom on
    /// its own does not parse as a Predicate (confirmed empirically: it
    /// parses only via `parse_int_expression`'s atom fallback, whose public
    /// eval entry point is Int-typed and rejects a Bool root). Same
    /// wrap-in-`Compare` composition as `TextEquals`, specialised to the
    /// unquoted `true`/`false` literal grammar.
    BoolEquals {
        context: fn() -> EvaluationContext,
        rng: fn() -> DeterministicRng,
        expect: bool,
    },
    /// The example's result genuinely depends on state this crate cannot
    /// construct from its own public surface (e.g. a live world/db row this
    /// crate never reads). Kept RARE by design (D5) — the gate reports these
    /// by name rather than silently skipping them.
    NotEvaluatable { reason: &'static str },
}

fn empty_context() -> EvaluationContext {
    EvaluationContext::default()
}

fn no_draws() -> DeterministicRng {
    DeterministicRng::new(vec![])
}

/// Shared minimal fact context: one small, internally-consistent world
/// covering every plain equality-filtered leaf read/check example (flags,
/// stats, tags, capabilities, items, niches, statuses, traces, bindings,
/// event-detail text). Kept in ONE place so every example's expected value
/// is traceable to the same handful of authored facts, per design D5's
/// "shared minimal fact contexts" directive.
fn base_context() -> EvaluationContext {
    EvaluationContext {
        bool_fields: vec![SubjectBoolFact {
            subject: SubjectRef::Target,
            field_id: "is_open".into(),
            value: true,
        }],
        int_fields: vec![
            SubjectIntFact {
                subject: SubjectRef::Actor,
                field_id: "core/strength".into(),
                value: 15,
            },
            SubjectIntFact {
                subject: SubjectRef::Actor,
                field_id: "core/dexterity".into(),
                value: 15,
            },
        ],
        effective_int_fields: vec![SubjectIntFact {
            subject: SubjectRef::Actor,
            field_id: "homebrew/Momentum".into(),
            value: 7,
        }],
        resource_bounds: vec![SubjectResourceBoundFact {
            subject: SubjectRef::Actor,
            field_id: "srd/hp".into(),
            min_value: 0,
            max_value: 20,
        }],
        text_fields: vec![SubjectTextFact {
            subject: SubjectRef::Event(EventRef::Detail {
                field_id: "speech_act".into(),
            }),
            field_id: "speech_act".into(),
            value: "greet".into(),
        }],
        capabilities: vec![SubjectIdFact {
            subject: SubjectRef::Actor,
            id: "core/open".into(),
        }],
        items: vec![SubjectIdFact {
            subject: SubjectRef::Actor,
            id: "core/torch".into(),
        }],
        niches: vec![SubjectIdFact {
            subject: SubjectRef::Target,
            id: "core/container".into(),
        }],
        tags: vec![
            SubjectIdFact {
                subject: SubjectRef::Target,
                id: "locked".into(),
            },
            SubjectIdFact {
                subject: SubjectRef::Target,
                id: "chest".into(),
            },
            SubjectIdFact {
                subject: SubjectRef::Actor,
                id: "actor.language.common".into(),
            },
            SubjectIdFact {
                subject: SubjectRef::Target,
                id: "actor.language.common".into(),
            },
        ],
        statuses: vec![SubjectIdFact {
            subject: SubjectRef::Actor,
            id: "core/poisoned".into(),
        }],
        traces: vec![SubjectIdFact {
            subject: SubjectRef::Target,
            id: "core/opened".into(),
        }],
        known_entities: vec![SubjectIdFact {
            subject: SubjectRef::Actor,
            id: "42".into(),
        }],
        bindings: vec![BindingFact {
            name: "required_entity_id".into(),
            value: "42".into(),
        }],
        ..Default::default()
    }
}

fn participant_tag_group_count_context() -> EvaluationContext {
    EvaluationContext {
        participant_tag_group_counts: vec![ParticipantTagGroupCountFact {
            prefix: "side:".into(),
            excluding_condition_definition_id: Some("core/condition/unavailable".into()),
            count: 1,
        }],
        ..Default::default()
    }
}

fn occupants_at_context() -> EvaluationContext {
    // ADR-0134 D-3: one occupant on the destination cell, and it is dead —
    // the SRD step clause passes.
    EvaluationContext {
        occupants_at: vec![OccupantsAtFact {
            scope: "to_node".into(),
            excluding_condition_definition_id: Some("srd/condition/dead".into()),
            count: 0,
        }],
        ..Default::default()
    }
}

fn peers_near_target_context() -> EvaluationContext {
    EvaluationContext {
        peers_near_target: vec![PeersNearTargetFact {
            prefix: "srd.faction.".into(),
            radius_mm: 1524,
            excluding_condition_definition_id: Some("srd/condition/incapacitated".into()),
            count: 1,
        }],
        ..Default::default()
    }
}

fn channel_pool_context() -> EvaluationContext {
    EvaluationContext {
        channel_pools: vec![ChannelPoolFact {
            channel_id: "core/action_class/primary".into(),
            pool_ms_per_round: 1000,
        }],
        ..Default::default()
    }
}

fn roll_table_context() -> EvaluationContext {
    EvaluationContext {
        roll_table_entries: vec![RollTableEntryFact {
            table_id: "core/random_event".into(),
            entry_id: "e1".into(),
            weight: 1,
            result_ref: "7".into(),
        }],
        ..Default::default()
    }
}

fn dsl_source_context() -> EvaluationContext {
    EvaluationContext {
        item_dsl_sources: vec![ItemDslSourceFact {
            subject: SubjectRef::ActorWieldedItem,
            slot: "srd/damage".into(),
            expr: parse_int_expression("5").expect("literal item source parses"),
        }],
        ..Default::default()
    }
}

fn equipped_dsl_source_context() -> EvaluationContext {
    let mut context = dsl_source_context();
    context.item_dsl_sources.push(ItemDslSourceFact {
        subject: SubjectRef::ActorEquippedItem {
            slot_id: "demo/main-hand".into(),
        },
        slot: "demo/damage".into(),
        expr: parse_int_expression("9").expect("literal equipped source parses"),
    });
    context
}

/// `1d20`: single draw, `RawEntropy` source — face = `(19 % 20) + 1 = 20`.
fn rng_roll_1d20_max() -> DeterministicRng {
    DeterministicRng::new(vec![19])
}

/// `cond(d20 >= 13, 0, 0 - d6)`: the `d20` draw resolves to face 20 (>= 13),
/// so `cond` takes the `then` branch (`0`, a literal — no further draw).
fn rng_cond_then_branch() -> DeterministicRng {
    DeterministicRng::new(vec![19])
}

/// `roll_under(1d20, actor.stat('core/dexterity'))` against dexterity 15:
/// face = `(9 % 20) + 1 = 10`, and `10 <= 15` succeeds.
fn rng_roll_under_success() -> DeterministicRng {
    DeterministicRng::new(vec![9])
}

/// `let difficulty = 10 ; 1d20 >= difficulty`: face = `(14 % 20) + 1 = 15`,
/// and `15 >= 10` succeeds.
fn rng_let_roll_succeeds() -> DeterministicRng {
    DeterministicRng::new(vec![14])
}

/// `roll_table('core/random_event')` int draw: the context's single
/// weighted candidate is selected regardless of the draw's raw value, so any
/// draw resolves to the SAME entry (`result_ref: "7"`).
fn rng_single_candidate_roll_table() -> DeterministicRng {
    DeterministicRng::new(vec![0])
}

/// One documented expectation per `(operator, example)` pair. Every
/// `authoring: "expression"` example in `dsl_reference()` must have an entry
/// here — a new or changed example with no matching row fails the gate below
/// by name, rather than silently passing (D5 / the eval-proven requirement).
fn documented_expectations() -> Vec<(&'static str, &'static str, DslExampleExpect)> {
    vec![
        (
            "BoolLiteral",
            "true",
            DslExampleExpect::BoolEquals {
                context: empty_context,
                rng: no_draws,
                expect: true,
            },
        ),
        (
            "IntLiteral",
            "10",
            DslExampleExpect::Int {
                context: empty_context,
                rng: no_draws,
                expect: 10,
            },
        ),
        (
            "TextLiteral",
            "'opened'",
            DslExampleExpect::TextEquals {
                context: empty_context,
                rng: no_draws,
                expect: "opened",
            },
        ),
        (
            "ReadInt",
            "actor.stat('core/strength')",
            DslExampleExpect::Int {
                context: base_context,
                rng: no_draws,
                expect: 15,
            },
        ),
        (
            "ReadEffectiveInt",
            "actor.effective('homebrew/Momentum')",
            DslExampleExpect::Int {
                context: base_context,
                rng: no_draws,
                expect: 7,
            },
        ),
        (
            "ReadText",
            "$event.detail.speech_act",
            DslExampleExpect::TextEquals {
                context: base_context,
                rng: no_draws,
                expect: "greet",
            },
        ),
        (
            "ReadBool",
            "target.state('is_open')",
            DslExampleExpect::Predicate {
                context: base_context,
                rng: no_draws,
                expect: true,
            },
        ),
        (
            "ReadResourceMax",
            "actor.resource_max('srd/hp')",
            DslExampleExpect::Int {
                context: base_context,
                rng: no_draws,
                expect: 20,
            },
        ),
        (
            "ReadResourceMin",
            "actor.resource_min('srd/hp')",
            DslExampleExpect::Int {
                context: base_context,
                rng: no_draws,
                expect: 0,
            },
        ),
        (
            "HasCapability",
            "actor.has_capability('core/open')",
            DslExampleExpect::Predicate {
                context: base_context,
                rng: no_draws,
                expect: true,
            },
        ),
        (
            "HasItem",
            "actor.has_item('core/torch')",
            DslExampleExpect::Predicate {
                context: base_context,
                rng: no_draws,
                expect: true,
            },
        ),
        (
            "HasNiche",
            "target.has_niche('core/container')",
            DslExampleExpect::Predicate {
                context: base_context,
                rng: no_draws,
                expect: true,
            },
        ),
        (
            "HasTag",
            "target.has_tag('locked')",
            DslExampleExpect::Predicate {
                context: base_context,
                rng: no_draws,
                expect: true,
            },
        ),
        (
            "SharesTag",
            "shares_tag(actor, target, 'actor.language.')",
            DslExampleExpect::Predicate {
                context: base_context,
                rng: no_draws,
                expect: true,
            },
        ),
        (
            "HasStatus",
            "actor.status('core/poisoned') == true",
            DslExampleExpect::Predicate {
                context: base_context,
                rng: no_draws,
                expect: true,
            },
        ),
        (
            "ImpliedByCount",
            "implied_by_count('core/condition/incapacitated') == 0",
            DslExampleExpect::Predicate {
                // "The evaluator never reads database tables; empty context
                // returns 0" (dsl_reference note) — proven with NO facts.
                context: empty_context,
                rng: no_draws,
                expect: true,
            },
        ),
        (
            "ParticipantTagGroupCount",
            "participant_tag_group_count('side:', 'core/condition/unavailable') <= 1",
            DslExampleExpect::Predicate {
                context: participant_tag_group_count_context,
                rng: no_draws,
                expect: true,
            },
        ),
        (
            "PeersNearTarget",
            "peers_near_target('srd.faction.', 1524, 'srd/condition/incapacitated') >= 1",
            DslExampleExpect::Predicate {
                context: peers_near_target_context,
                rng: no_draws,
                expect: true,
            },
        ),
        (
            "OccupantsAt",
            "occupants_at('to_node', 'srd/condition/dead') == 0",
            DslExampleExpect::Predicate {
                context: occupants_at_context,
                rng: no_draws,
                expect: true,
            },
        ),
        (
            "ChannelPool",
            "channel_pool('core/action_class/primary')",
            DslExampleExpect::Int {
                context: channel_pool_context,
                rng: no_draws,
                expect: 1000,
            },
        ),
        (
            "TracePresent",
            "target.trace('core/opened')",
            DslExampleExpect::Predicate {
                context: base_context,
                rng: no_draws,
                expect: true,
            },
        ),
        (
            "KnowsEntity",
            "actor.knows_entity($bindings.required_entity_id)",
            DslExampleExpect::Predicate {
                context: base_context,
                rng: no_draws,
                expect: true,
            },
        ),
        (
            "And",
            "actor.has_capability('core/open') && target.has_tag('locked')",
            DslExampleExpect::Predicate {
                context: base_context,
                rng: no_draws,
                expect: true,
            },
        ),
        (
            "Or",
            "target.has_tag('door') || target.has_tag('chest')",
            DslExampleExpect::Predicate {
                // `door` is absent, `chest` is present — proves the
                // short-circuit-to-true branch, not a vacuous both-true case.
                context: base_context,
                rng: no_draws,
                expect: true,
            },
        ),
        (
            "Not",
            "target.has_tag('sealed') == false",
            DslExampleExpect::Predicate {
                // `sealed` is absent from every context; `HasTag` fails
                // closed to `false` on a missing fact (ADR-0035), so this
                // holds with NO facts.
                context: empty_context,
                rng: no_draws,
                expect: true,
            },
        ),
        (
            "Compare",
            "actor.stat('core/strength') >= 10",
            DslExampleExpect::Predicate {
                context: base_context,
                rng: no_draws,
                expect: true,
            },
        ),
        (
            "Arithmetic",
            "max(actor.stat('core/strength'), 2)",
            DslExampleExpect::Int {
                context: base_context,
                rng: no_draws,
                expect: 15,
            },
        ),
        (
            "Cond",
            "cond(d20 >= 13, 0, 0 - d6)",
            DslExampleExpect::Int {
                context: empty_context,
                rng: rng_cond_then_branch,
                expect: 0,
            },
        ),
        (
            "Roll",
            "1d20",
            DslExampleExpect::Int {
                context: empty_context,
                rng: rng_roll_1d20_max,
                expect: 20,
            },
        ),
        (
            "RollUnder",
            "roll_under(1d20, actor.stat('core/dexterity'))",
            DslExampleExpect::Predicate {
                context: base_context,
                rng: rng_roll_under_success,
                expect: true,
            },
        ),
        (
            "RollTable",
            "roll_table('core/random_event')",
            DslExampleExpect::Int {
                context: roll_table_context,
                rng: rng_single_candidate_roll_table,
                expect: 7,
            },
        ),
        (
            "DslSource",
            "actor.wielded_item.source('srd/damage')",
            DslExampleExpect::Int {
                context: dsl_source_context,
                rng: no_draws,
                expect: 5,
            },
        ),
        (
            "DslSource",
            "actor.equipped_item('demo/main-hand').source('demo/damage')",
            DslExampleExpect::Int {
                context: equipped_dsl_source_context,
                rng: no_draws,
                expect: 9,
            },
        ),
        (
            "Let",
            "let difficulty = 10 ; 1d20 >= difficulty",
            DslExampleExpect::Predicate {
                context: empty_context,
                rng: rng_let_roll_succeeds,
                expect: true,
            },
        ),
        (
            "OutcomeSequence",
            "sequence(invoke_outcome_graph('core/open_container'), invoke_outcome_graph('core/emit_noise'))",
            DslExampleExpect::NotEvaluatable {
                reason: "outcome expressions execute in the server (`outcome_execution.rs`); this crate exposes no outcome evaluator",
            },
        ),
        (
            "InvokeOutcomeGraph",
            "invoke_outcome_graph('core/open_container')",
            DslExampleExpect::NotEvaluatable {
                reason: "outcome expressions execute in the server (`outcome_execution.rs`); this crate exposes no outcome evaluator",
            },
        ),
        (
            "Arbitrate",
            "arbitrate('core/dialogue_choice', 'mood=tense', 'agree=core/dialogue/agree', 'refuse=core/dialogue/refuse')",
            DslExampleExpect::NotEvaluatable {
                reason: "outcome expressions execute in the server (`outcome_execution.rs`); this crate exposes no outcome evaluator",
            },
        ),
    ]
}

/// Every `authoring: "expression"` operator example evaluates — through the
/// SAME public entry points production callers use — to the result its
/// `documented_expectations()` row declares. This is the eval half of the
/// executable-documentation guard `reference_examples.rs` starts (parse
/// only); together they prove a documented expression example neither
/// drifts out of the grammar NOR out of its claimed behaviour.
#[test]
fn every_expression_example_evaluates_to_its_documented_result() {
    let reference = dsl_reference();
    let expectations = documented_expectations();
    let limits = EvaluationLimits::default();
    let mut failures = Vec::new();
    let mut not_evaluatable = Vec::new();

    for op in &reference.operators {
        if op.authoring != "expression" {
            continue;
        }
        for example in &op.examples {
            let Some((_, _, expect)) = expectations
                .iter()
                .find(|(operator, source, _)| operator == &op.operator && source == example)
            else {
                failures.push(format!(
                    "{} :: {example:?} — no documented expectation in documented_expectations()",
                    op.operator
                ));
                continue;
            };

            match expect {
                DslExampleExpect::Predicate {
                    context,
                    rng,
                    expect,
                } => {
                    let parsed = match parse_predicate(example) {
                        Ok(expr) => expr,
                        Err(err) => {
                            failures.push(format!(
                                "{} :: {example:?} — expected a Predicate, failed to parse: {err:?}",
                                op.operator
                            ));
                            continue;
                        }
                    };
                    let facts = context();
                    let mut rng = rng();
                    match evaluate_predicate_expression(&parsed, &facts, &mut rng, limits) {
                        Ok(actual) if actual == *expect => {}
                        Ok(actual) => failures.push(format!(
                            "{} :: {example:?} — expected {expect:?}, evaluated to {actual:?}",
                            op.operator
                        )),
                        Err(err) => failures.push(format!(
                            "{} :: {example:?} — expected {expect:?}, evaluation errored: {err:?}",
                            op.operator
                        )),
                    }
                }
                DslExampleExpect::Int {
                    context,
                    rng,
                    expect,
                } => {
                    let parsed = match parse_int_expression(example) {
                        Ok(expr) => expr,
                        Err(err) => {
                            failures.push(format!(
                                "{} :: {example:?} — expected an Int expression, failed to parse: {err:?}",
                                op.operator
                            ));
                            continue;
                        }
                    };
                    let facts = context();
                    let mut rng = rng();
                    match evaluate_int_expression(&parsed, &facts, &mut rng, limits) {
                        Ok(actual) if actual == *expect => {}
                        Ok(actual) => failures.push(format!(
                            "{} :: {example:?} — expected {expect}, evaluated to {actual}",
                            op.operator
                        )),
                        Err(err) => failures.push(format!(
                            "{} :: {example:?} — expected {expect}, evaluation errored: {err:?}",
                            op.operator
                        )),
                    }
                }
                DslExampleExpect::TextEquals {
                    context,
                    rng,
                    expect,
                } => {
                    let wrapped = format!("{example} == '{expect}'");
                    let parsed = match parse_predicate(&wrapped) {
                        Ok(expr) => expr,
                        Err(err) => {
                            failures.push(format!(
                                "{} :: {example:?} — expected Text {expect:?}, wrapped comparison {wrapped:?} failed to parse: {err:?}",
                                op.operator
                            ));
                            continue;
                        }
                    };
                    let facts = context();
                    let mut rng = rng();
                    match evaluate_predicate_expression(&parsed, &facts, &mut rng, limits) {
                        Ok(true) => {}
                        Ok(false) => failures.push(format!(
                            "{} :: {example:?} — did not evaluate to the documented text {expect:?}",
                            op.operator
                        )),
                        Err(err) => failures.push(format!(
                            "{} :: {example:?} — expected text {expect:?}, evaluation errored: {err:?}",
                            op.operator
                        )),
                    }
                }
                DslExampleExpect::BoolEquals {
                    context,
                    rng,
                    expect,
                } => {
                    let wrapped = format!("{example} == {expect}");
                    let parsed = match parse_predicate(&wrapped) {
                        Ok(expr) => expr,
                        Err(err) => {
                            failures.push(format!(
                                "{} :: {example:?} — expected Bool {expect}, wrapped comparison {wrapped:?} failed to parse: {err:?}",
                                op.operator
                            ));
                            continue;
                        }
                    };
                    let facts = context();
                    let mut rng = rng();
                    match evaluate_predicate_expression(&parsed, &facts, &mut rng, limits) {
                        Ok(true) => {}
                        Ok(false) => failures.push(format!(
                            "{} :: {example:?} — did not evaluate to the documented bool {expect}",
                            op.operator
                        )),
                        Err(err) => failures.push(format!(
                            "{} :: {example:?} — expected bool {expect}, evaluation errored: {err:?}",
                            op.operator
                        )),
                    }
                }
                DslExampleExpect::NotEvaluatable { reason } => {
                    not_evaluatable.push(format!("{} :: {example:?} — {reason}", op.operator));
                }
            }
        }
    }

    assert!(
        failures.is_empty(),
        "expression-authored operator examples that did not evaluate to their documented result:\n{}\n\nnot-evaluatable (informational, not failures):\n{}",
        failures.join("\n"),
        not_evaluatable.join("\n"),
    );
}

/// Guards `documented_expectations()` itself against drift: every row must
/// name a real `(operator, example)` pair still present in `dsl_reference()`
/// — a stale row (operator renamed, example text changed) is caught here
/// instead of silently never being exercised by the test above.
#[test]
fn every_documented_expectation_matches_a_live_reference_example() {
    let reference = dsl_reference();
    let stale: Vec<String> = documented_expectations()
        .into_iter()
        .filter(|(operator, source, _)| {
            !reference.operators.iter().any(|op| {
                op.authoring == "expression"
                    && &op.operator == operator
                    && op.examples.iter().any(|example| example == source)
            })
        })
        .map(|(operator, source, _)| format!("{operator} :: {source:?}"))
        .collect();
    assert!(
        stale.is_empty(),
        "documented_expectations() rows with no matching live reference example (stale — drifted \
         operator name or example text): {stale:?}"
    );
}

#[test]
fn traced_and_untraced_evaluation_agree_on_all_reference_examples() {
    let reference = dsl_reference();
    let expectations = documented_expectations();
    let limits = EvaluationLimits::default();
    let mut failures = Vec::new();

    for op in &reference.operators {
        if op.authoring != "expression" {
            continue;
        }
        for example in &op.examples {
            let Some((_, _, expect)) = expectations
                .iter()
                .find(|(operator, source, _)| operator == &op.operator && source == example)
            else {
                continue;
            };

            match expect {
                DslExampleExpect::Predicate {
                    context,
                    rng,
                    expect: _,
                } => {
                    let Ok(parsed) = parse_predicate(example) else {
                        continue;
                    };
                    let facts = context();
                    let mut rng_untraced = rng();
                    let mut rng_traced = rng();
                    let untraced = evaluate_predicate_expression_with_report(
                        &parsed,
                        &facts,
                        &mut rng_untraced,
                        limits,
                    );
                    let traced =
                        evaluate_expression_traced(&parsed, &facts, &mut rng_traced, limits);
                    match (untraced, traced) {
                        (Ok(u), Ok(t)) => {
                            if t.value != EvaluationValue::Bool(u.value) {
                                failures.push(format!(
                                    "{} :: {example:?} — value mismatch: untraced {:?}, traced {:?}",
                                    op.operator, u.value, t.value
                                ));
                            }
                            if t.stochastic_draws != u.stochastic_draws {
                                failures.push(format!(
                                    "{} :: {example:?} — draws mismatch: untraced {:?}, traced {:?}",
                                    op.operator, u.stochastic_draws, t.stochastic_draws
                                ));
                            }
                            if t.draws_used != u.draws_used {
                                failures.push(format!(
                                    "{} :: {example:?} — draws_used mismatch: untraced {}, traced {}",
                                    op.operator, u.draws_used, t.draws_used
                                ));
                            }
                            if t.trace.is_empty() {
                                failures.push(format!(
                                    "{} :: {example:?} — traced evaluation returned empty trace",
                                    op.operator
                                ));
                            }
                        }
                        (Err(u_err), Err(t_err)) => {
                            if u_err != t_err.error {
                                failures.push(format!(
                                    "{} :: {example:?} — error mismatch: untraced {:?}, traced {:?}",
                                    op.operator, u_err, t_err.error
                                ));
                            }
                        }
                        (u, t) => {
                            failures.push(format!(
                                "{} :: {example:?} — Ok/Err mismatch: untraced is_ok={}, traced is_ok={}",
                                op.operator, u.is_ok(), t.is_ok()
                            ));
                        }
                    }
                }
                DslExampleExpect::Int {
                    context,
                    rng,
                    expect: _,
                } => {
                    let Ok(parsed) = parse_int_expression(example) else {
                        continue;
                    };
                    let facts = context();
                    let mut rng_untraced = rng();
                    let mut rng_traced = rng();
                    let untraced =
                        evaluate_int_expression(&parsed, &facts, &mut rng_untraced, limits);
                    let traced =
                        evaluate_expression_traced(&parsed, &facts, &mut rng_traced, limits);
                    match (untraced, traced) {
                        (Ok(u), Ok(t)) => {
                            if t.value != EvaluationValue::Int(u) {
                                failures.push(format!(
                                    "{} :: {example:?} — value mismatch: untraced {u}, traced {:?}",
                                    op.operator, t.value
                                ));
                            }
                            if t.trace.is_empty() {
                                failures.push(format!(
                                    "{} :: {example:?} — traced evaluation returned empty trace",
                                    op.operator
                                ));
                            }
                        }
                        (Err(u_err), Err(t_err)) => {
                            if u_err != t_err.error {
                                failures.push(format!(
                                    "{} :: {example:?} — error mismatch: untraced {:?}, traced {:?}",
                                    op.operator, u_err, t_err.error
                                ));
                            }
                        }
                        (u, t) => {
                            failures.push(format!(
                                "{} :: {example:?} — Ok/Err mismatch: untraced is_ok={}, traced is_ok={}",
                                op.operator, u.is_ok(), t.is_ok()
                            ));
                        }
                    }
                }
                DslExampleExpect::TextEquals {
                    context,
                    rng,
                    expect,
                } => {
                    let Ok(parsed) = compile_text_expression_with_macros(example, &[]) else {
                        continue;
                    };
                    let facts = context();
                    let mut rng_traced = rng();
                    let traced =
                        evaluate_expression_traced(&parsed, &facts, &mut rng_traced, limits);
                    match traced {
                        Ok(t) => {
                            if t.value != EvaluationValue::Text(expect.to_string()) {
                                failures.push(format!(
                                    "{} :: {example:?} — text value mismatch: expected {expect:?}, got {:?}",
                                    op.operator, t.value
                                ));
                            }
                            if t.trace.is_empty() {
                                failures.push(format!(
                                    "{} :: {example:?} — traced evaluation returned empty trace",
                                    op.operator
                                ));
                            }
                        }
                        Err(t_err) => {
                            failures.push(format!(
                                "{} :: {example:?} — text evaluation failed: {:?}",
                                op.operator, t_err.error
                            ));
                        }
                    }
                }
                DslExampleExpect::BoolEquals {
                    context,
                    rng,
                    expect,
                } => {
                    let wrapped = format!("{example} == {expect}");
                    let Ok(parsed) = parse_predicate(&wrapped) else {
                        continue;
                    };
                    let facts = context();
                    let mut rng_untraced = rng();
                    let mut rng_traced = rng();
                    let untraced = evaluate_predicate_expression_with_report(
                        &parsed,
                        &facts,
                        &mut rng_untraced,
                        limits,
                    );
                    let traced =
                        evaluate_expression_traced(&parsed, &facts, &mut rng_traced, limits);
                    match (untraced, traced) {
                        (Ok(u), Ok(t)) => {
                            if t.value != EvaluationValue::Bool(u.value) {
                                failures.push(format!(
                                    "{} :: {example:?} — value mismatch: untraced {:?}, traced {:?}",
                                    op.operator, u.value, t.value
                                ));
                            }
                            if t.stochastic_draws != u.stochastic_draws {
                                failures.push(format!(
                                    "{} :: {example:?} — draws mismatch: untraced {:?}, traced {:?}",
                                    op.operator, u.stochastic_draws, t.stochastic_draws
                                ));
                            }
                        }
                        (Err(u_err), Err(t_err)) => {
                            if u_err != t_err.error {
                                failures.push(format!(
                                    "{} :: {example:?} — error mismatch: untraced {:?}, traced {:?}",
                                    op.operator, u_err, t_err.error
                                ));
                            }
                        }
                        (u, t) => {
                            failures.push(format!(
                                "{} :: {example:?} — Ok/Err mismatch: untraced is_ok={}, traced is_ok={}",
                                op.operator, u.is_ok(), t.is_ok()
                            ));
                        }
                    }
                }
                DslExampleExpect::NotEvaluatable { .. } => {}
            }
        }
    }

    assert!(
        failures.is_empty(),
        "traced and untraced evaluation disagreed on reference examples:\n{}",
        failures.join("\n")
    );
}
