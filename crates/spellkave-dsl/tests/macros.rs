use spellkave_dsl::{
    compile_int_expression_with_macros, compile_predicate_with_macros,
    compile_text_expression_with_macros, evaluate_int_expression, DeterministicRng,
    DslMacroDefinition, DslMacroParameter, DslValueType, EvaluationContext, EvaluationLimits, Op,
    SubjectIntFact, SubjectRef,
};

fn ability_modifier() -> DslMacroDefinition {
    DslMacroDefinition {
        macro_id: "srd/ability_modifier".to_string(),
        return_type: DslValueType::Int,
        parameters: vec![DslMacroParameter {
            name: "score".to_string(),
            value_type: DslValueType::Int,
        }],
        body_source: "(score - 10) / 2".to_string(),
    }
}

#[test]
fn int_macro_call_expands_to_existing_ops_only() {
    let expression = compile_int_expression_with_macros(
        "macro('srd/ability_modifier', actor.stat('srd/dex'))",
        &[ability_modifier()],
    )
    .expect("macro call should compile");

    assert!(expression
        .nodes
        .iter()
        .any(|node| matches!(node.op, Op::Arithmetic(_))));
    assert!(expression
        .nodes
        .iter()
        .any(|node| matches!(node.op, Op::ReadInt(_))));
    assert!(expression
        .nodes
        .iter()
        .all(|node| node.op.variant_name() != "MacroCall"));

    let facts = EvaluationContext {
        int_fields: vec![SubjectIntFact {
            subject: SubjectRef::Actor,
            field_id: "srd/dex".to_string(),
            value: 16,
        }],
        ..EvaluationContext::default()
    };
    let value = evaluate_int_expression(
        &expression,
        &facts,
        &mut DeterministicRng::new(vec![]),
        EvaluationLimits::default(),
    )
    .expect("the unchanged evaluator dispatch accepts the expanded AST");
    assert_eq!(value, 3);
}

#[test]
fn leaf_macros_compose_at_the_call_site() {
    let base_ac = DslMacroDefinition {
        macro_id: "srd/base_ac".to_string(),
        return_type: DslValueType::Int,
        parameters: vec![DslMacroParameter {
            name: "dex_modifier".to_string(),
            value_type: DslValueType::Int,
        }],
        body_source: "10 + dex_modifier".to_string(),
    };
    let expression = compile_int_expression_with_macros(
        "macro('srd/base_ac', macro('srd/ability_modifier', actor.stat('srd/dex')))",
        &[ability_modifier(), base_ac],
    )
    .expect("leaf macros should compose at an authored call site");

    let facts = EvaluationContext {
        int_fields: vec![SubjectIntFact {
            subject: SubjectRef::Actor,
            field_id: "srd/dex".to_string(),
            value: 16,
        }],
        ..EvaluationContext::default()
    };
    let value = evaluate_int_expression(
        &expression,
        &facts,
        &mut DeterministicRng::new(vec![]),
        EvaluationLimits::default(),
    )
    .expect("the composed leaves expand to the unchanged evaluator");
    assert_eq!(value, 13);
}

#[test]
fn macro_introduced_reads_and_rolls_remain_visible_to_existing_analysis() {
    let definitions = [DslMacroDefinition {
        macro_id: "srd/rolled_target_score".to_string(),
        return_type: DslValueType::Int,
        parameters: vec![],
        body_source: "2d6 + target.stat('srd/dc')".to_string(),
    }];
    let expression =
        compile_int_expression_with_macros("macro('srd/rolled_target_score')", &definitions)
            .expect("macro call should expand");

    assert!(expression
        .nodes
        .iter()
        .any(|node| matches!(node.op, Op::Roll(_))));
    assert!(expression.nodes.iter().any(|node| {
        matches!(
            &node.op,
            Op::ReadInt(read) if read.subject == SubjectRef::Target
        )
    }));
    assert_eq!(EvaluationLimits::for_expression(&expression).max_draws, 4);
}

#[test]
fn zero_argument_macro_reads_the_actor_effective_overlay() {
    let definitions = [DslMacroDefinition {
        macro_id: "srd/strength_modifier".to_string(),
        return_type: DslValueType::Int,
        parameters: vec![],
        body_source: "((actor.effective('srd/str')) - 10) / 2 + cond(((actor.effective('srd/str')) - 10) - (((actor.effective('srd/str')) - 10) / 2) * 2 < 0, -1, 0)".to_string(),
    }];
    let expression =
        compile_int_expression_with_macros("macro('srd/strength_modifier')", &definitions)
            .expect("zero-argument actor macro should compile");
    assert!(expression
        .nodes
        .iter()
        .any(|node| matches!(node.op, Op::ReadEffectiveInt(_))));
    let facts = EvaluationContext {
        int_fields: vec![SubjectIntFact {
            subject: SubjectRef::Actor,
            field_id: "srd/str".to_string(),
            value: 10,
        }],
        effective_int_fields: vec![SubjectIntFact {
            subject: SubjectRef::Actor,
            field_id: "srd/str".to_string(),
            value: 17,
        }],
        ..EvaluationContext::default()
    };
    let value = evaluate_int_expression(
        &expression,
        &facts,
        &mut DeterministicRng::new(vec![]),
        EvaluationLimits::default(),
    )
    .expect("effective actor score should evaluate");
    assert_eq!(value, 3);
}

#[test]
fn text_macro_preserves_the_existing_text_value_type() {
    let definitions = [DslMacroDefinition {
        macro_id: "core/label".to_string(),
        return_type: DslValueType::Text,
        parameters: vec![],
        body_source: "'label'".to_string(),
    }];
    let expression = compile_text_expression_with_macros("macro('core/label')", &definitions)
        .expect("text macro call should compile");
    assert_eq!(expression.nodes[0].value_type, DslValueType::Text);
}

#[test]
fn bool_macro_call_uses_its_declared_type() {
    let definitions = [DslMacroDefinition {
        macro_id: "core/is_marked".to_string(),
        return_type: DslValueType::Bool,
        parameters: vec![],
        body_source: "actor.has_tag('core/marked')".to_string(),
    }];
    let expression = compile_predicate_with_macros("macro('core/is_marked')", &definitions)
        .expect("bool macro call should compile");
    assert_eq!(expression.kind, spellkave_dsl::ExpressionKind::Predicate);
    assert!(expression
        .nodes
        .iter()
        .any(|node| matches!(node.op, Op::HasTag(_))));
}

#[test]
fn macro_calls_in_bodies_are_rejected_before_expansion() {
    let definitions = [DslMacroDefinition {
        macro_id: "core/wrapper".to_string(),
        return_type: DslValueType::Int,
        parameters: vec![],
        body_source: "macro('core/other')".to_string(),
    }];
    let error = compile_int_expression_with_macros("macro('core/wrapper')", &definitions)
        .expect_err("macro bodies cannot call macros in v1");
    assert_eq!(error.diagnostics[0].code, "MF-DSL-MACRO-IN-BODY");
}

#[test]
fn unknown_macro_has_a_call_site_diagnostic() {
    let error = compile_int_expression_with_macros("macro('missing/id', 1)", &[])
        .expect_err("unknown macro must fail");
    assert_eq!(error.diagnostics[0].code, "MF-DSL-MACRO-UNKNOWN");
}

#[test]
fn macro_argument_type_mismatch_is_rejected_at_the_call() {
    let error = compile_int_expression_with_macros(
        "macro('srd/ability_modifier', true)",
        &[ability_modifier()],
    )
    .expect_err("Bool does not satisfy an Int parameter");
    assert_eq!(error.diagnostics[0].code, "MF-DSL-MACRO-ARG-TYPE");
}
