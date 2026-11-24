use spellkave_dsl::{
    evaluate_int_expression, parse_int_expression, parse_predicate, DeterministicRng,
    EvaluationContext, EvaluationLimits, ItemDslSourceFact, SubjectIntFact, SubjectRef,
};

#[test]
fn absent_slot_facts_do_not_invent_tag_integer_text_or_source_values() {
    let facts = EvaluationContext::default();
    let limits = EvaluationLimits::default();
    let tag =
        parse_predicate("actor.equipped_item('demo/absent').has_tag('demo/cutting')").unwrap();
    assert_eq!(
        spellkave_dsl::evaluate_predicate_expression(
            &tag,
            &facts,
            &mut DeterministicRng::new(vec![]),
            limits,
        ),
        Ok(false)
    );
    for source in [
        "actor.equipped_item('demo/absent').stat('demo/value')",
        "actor.equipped_item('demo/absent').source('demo/value')",
        "roll_table(actor.equipped_item('demo/absent').text('demo/table'))",
    ] {
        let expression = parse_int_expression(source).unwrap();
        assert!(
            evaluate_int_expression(
                &expression,
                &facts,
                &mut DeterministicRng::new(vec![]),
                limits,
            )
            .is_err(),
            "missing facts must fail: {source}"
        );
    }
}

#[test]
fn source_pointer_grammar_uses_literal_or_supported_subject_text() {
    for pointer in [
        "'demo/formula'",
        "actor.text('demo/pointer')",
        "target.text('demo/pointer')",
        "actor.equipped_item('demo/grip').text('demo/pointer')",
    ] {
        let source = format!("actor.equipped_item('demo/grip').source({pointer})");
        assert!(parse_int_expression(&source).is_ok(), "{source}");
    }
    for pointer in [
        "event.text('kind')",
        "bindings.formula.text('value')",
        "formula",
    ] {
        let source = format!("actor.equipped_item('demo/grip').source({pointer})");
        assert!(parse_int_expression(&source).is_err(), "{source}");
    }
}

#[test]
fn source_offer_stays_attemptable_while_execution_reads_changed_slot_state() {
    let predicate =
        parse_predicate("0 < actor.equipped_item('demo/grip').source('demo/value')").unwrap();
    for charge in [3, 0, 3] {
        let subject = SubjectRef::ActorEquippedItem {
            slot_id: "demo/grip".into(),
        };
        let facts = EvaluationContext {
            int_fields: vec![SubjectIntFact {
                subject: subject.clone(),
                field_id: "demo/charge".into(),
                value: charge,
            }],
            item_dsl_sources: vec![ItemDslSourceFact {
                subject,
                slot: "demo/value".into(),
                expr: parse_int_expression("actor.equipped_item('demo/grip').stat('demo/charge')")
                    .unwrap(),
            }],
            ..EvaluationContext::default()
        };
        assert_eq!(
            spellkave_dsl::evaluate_offer_predicate_expression(
                &predicate,
                &facts,
                EvaluationLimits::default()
            ),
            Ok(true)
        );
        assert_eq!(
            spellkave_dsl::evaluate_predicate_expression(
                &predicate,
                &facts,
                &mut DeterministicRng::new(vec![]),
                EvaluationLimits::default()
            ),
            Ok(charge > 0)
        );
    }
}

#[test]
fn equipment_dependencies_include_tags_text_sources_and_both_shared_subjects() {
    for (source, expected) in [
        (
            "actor.equipped_item('demo/a').has_tag('demo/tag')",
            vec!["demo/a"],
        ),
        (
            "roll_table(actor.equipped_item('demo/b').text('demo/profile')) > 0",
            vec!["demo/b"],
        ),
        (
            "shares_tag(actor.equipped_item('demo/a'), actor.equipped_item('demo/b'), 'demo/')",
            vec!["demo/a", "demo/b"],
        ),
        ("actor.wielded_item.has_tag('demo/tag')", vec![]),
    ] {
        let expression = parse_predicate(source).unwrap();
        let slots: Vec<_> = expression
            .nodes
            .iter()
            .flat_map(|node| node.op.equipped_item_slots())
            .collect();
        assert_eq!(slots, expected, "{source}");
    }
    let expression =
        parse_int_expression("actor.equipped_item('demo/c').source('demo/value')").unwrap();
    assert_eq!(
        expression
            .nodes
            .iter()
            .flat_map(|node| node.op.equipped_item_slots())
            .collect::<Vec<_>>(),
        vec!["demo/c"]
    );
}

#[test]
fn slot_source_retains_exact_subject_and_surrounding_actor() {
    let subject = |slot: &str| SubjectRef::ActorEquippedItem {
        slot_id: slot.into(),
    };
    let facts = EvaluationContext {
        int_fields: vec![SubjectIntFact {
            subject: SubjectRef::Actor,
            field_id: "demo/power".into(),
            value: 7,
        }],
        item_dsl_sources: vec![
            ItemDslSourceFact {
                subject: subject("demo/cover"),
                slot: "demo/value".into(),
                expr: parse_int_expression("99").unwrap(),
            },
            ItemDslSourceFact {
                subject: subject("demo/clamp"),
                slot: "demo/value".into(),
                expr: parse_int_expression("actor.stat('demo/power') + 2").unwrap(),
            },
        ],
        ..EvaluationContext::default()
    };
    let expression =
        parse_int_expression("actor.equipped_item('demo/clamp').source('demo/value')").unwrap();
    assert_eq!(
        evaluate_int_expression(
            &expression,
            &facts,
            &mut DeterministicRng::new(vec![]),
            EvaluationLimits::default(),
        ),
        Ok(9)
    );
}

#[test]
fn relation_requires_a_nonempty_literal_and_actor_root() {
    for source in [
        "actor.equipped_item('').stat('demo/x')",
        "actor.equipped_item('   ').stat('demo/x')",
        "actor.equipped_item(actor.text('demo/slot')).stat('demo/x')",
        "actor.equipped_item('demo/clamp', 'extra').stat('demo/x')",
        "target.equipped_item('demo/clamp').stat('demo/x')",
        "actor.equipped_item('demo/clamp')",
    ] {
        assert!(parse_int_expression(source).is_err(), "accepted {source}");
    }
    assert!(parse_predicate("actor.equipped_item('').has_tag('demo/cutting')").is_err());
}

#[test]
fn exact_slot_subject_survives_ast_serialization_without_changing_legacy() {
    for source in [
        "actor.wielded_item.has_tag('demo/cutting')",
        "actor.equipped_item('demo/clamp').has_tag('demo/cutting')",
        "roll_table(actor.equipped_item('demo/clamp').text('demo/profile')) > 0",
    ] {
        let expression = parse_predicate(source).unwrap();
        let json = serde_json::to_string(&expression).unwrap();
        assert_eq!(
            serde_json::from_str::<spellkave_dsl::Expression>(&json).unwrap(),
            expression
        );
    }
    assert_eq!(
        serde_json::to_string(&SubjectRef::ActorWieldedItem).unwrap(),
        "\"ActorWieldedItem\""
    );
}
