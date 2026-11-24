use spellkave_dsl::{
    evaluate_int_expression, parse_int_expression, parse_outcome, parse_predicate,
    validate_expression_for_insert, validate_spatial_reference_for_insert, DeterministicRng,
    EvaluationContext, EvaluationLimits, Expression, ExpressionKind, H3CellRef,
    LocationDefinitionRef, Op, PlaceEdgeRef, PlaceNodeRef, SpatialReference,
    SpatialReferenceValidationError, SubjectRef, TileRef, ValueType,
};

// ── helpers ──────────────────────────────────────────────────────────────────

fn node(node_id: u32, value_type: ValueType, op: Op) -> spellkave_dsl::Node {
    spellkave_dsl::Node {
        node_id,
        value_type,
        op,
        span: spellkave_dsl::SourceSpan::default(),
    }
}

// ── existing predicate surface ────────────────────────────────────────────────

#[test]
fn parses_existing_predicate_examples() {
    let movement = r#"actor.has_capability("core/walk") &&
target.has_niche("core/passable_portal") &&
target.state("open") == true &&
actor.status("immobilized") == false"#;
    let parsed = parse_predicate(movement).expect("movement fixture should parse");
    let canonical = parsed.canonical_json().expect("canonical json");
    assert!(canonical.contains("\"Predicate\""));
    assert!(canonical.contains("HasCapability"));
    assert!(canonical.contains("HasNiche"));

    let locked = r#"actor.has_item("core/item/cellar_key") &&
actor.has_item($bindings.required_key)"#;
    let parsed = parse_predicate(locked).expect("locked traversal item fixture should parse");
    let canonical = parsed.canonical_json().expect("canonical json");
    assert!(canonical.contains("HasItem"));
    assert!(canonical.contains("core/item/cellar_key"));
    assert!(canonical.contains("$bindings.required_key"));

    let arithmetic = r#"roll_under(d20, actor.stat("dex")) &&
max(0, actor.stat("core/strength") - target.armor) >= 1"#;
    let parsed = parse_predicate(arithmetic).expect("arithmetic fixture should parse");
    let canonical = parsed.canonical_json().expect("canonical json");
    assert!(canonical.contains("RollUnder"));
    assert!(canonical.contains("\"Max\""));
}

#[test]
fn parses_world_tick_read() {
    let parsed = parse_predicate("world.tick >= 0").expect("world.tick should parse");
    let canonical = parsed.canonical_json().expect("canonical json");
    assert!(canonical.contains("core/tick"));
    assert!(canonical.contains("\"World\""));
}

#[test]
fn parses_world_tick_arithmetic_expression() {
    let source = "world.tick - actor.last_rest_at_tick >= 480";
    let parsed = parse_predicate(source).expect("world tick arithmetic should parse");
    let canonical = parsed.canonical_json().expect("canonical json");
    assert!(canonical.contains("\"Sub\""));
    assert!(canonical.contains("core/tick"));
    assert!(canonical.contains("last_rest_at_tick"));
}

#[test]
fn parses_outcome_with_same_surface() {
    let source = r#"actor.has_capability("core/walk")"#;
    let parsed = parse_outcome(source).expect("outcome fixture should parse");
    let canonical = parsed.canonical_json().expect("canonical json");
    assert!(canonical.contains("\"Outcome\""));
}

#[test]
fn emits_stable_diagnostics_for_invalid_expressions() {
    let err = parse_predicate(r#"actor.can_fly("core/fly")"#).expect_err("must fail");
    assert_eq!(err.diagnostics[0].code, "MF-DSL-UNKNOWN-PRIMITIVE");
    assert_eq!(err.diagnostics[0].span.start, 6);
    assert_eq!(err.diagnostics[0].span.end, 13);

    let err = parse_predicate(r#"actor.has_capability("core/walk") ???"#).expect_err("must fail");
    assert_eq!(err.diagnostics[0].code, "MF-DSL-UNKNOWN-CHAR");
    assert_eq!(err.diagnostics[0].span.start, 34);
    assert_eq!(err.diagnostics[0].span.end, 35);
}

// ── Or / union grammar ────────────────────────────────────────────────────────

#[test]
fn parses_or_predicate() {
    let source = r#"actor.has_capability("core/walk") || actor.has_capability("core/swim")"#;
    let parsed = parse_predicate(source).expect("or predicate should parse");
    let canonical = parsed.canonical_json().expect("canonical json");
    assert!(canonical.contains("\"Or\""));
    assert!(canonical.contains("HasCapability"));
}

#[test]
fn parses_and_or_precedence() {
    // `a && b || c` — `||` binds TIGHTER than `&&` (`parse_and` is built over
    // `parse_or`), so the parser produces And(a, Or(b, c)). The root-shape
    // proof lives in `or_binds_tighter_than_and` below.
    let source = r#"actor.has_capability("core/walk") &&
actor.has_niche("core/swim") ||
target.state("open") == true"#;
    let parsed = parse_predicate(source).expect("and-or precedence should parse");
    let canonical = parsed.canonical_json().expect("canonical json");
    assert!(canonical.contains("\"Or\""));
    assert!(canonical.contains("\"And\""));
}

/// Operator precedence:
/// `a && b || c` is `a && (b || c)`, and boolean terms cannot be parenthesised.
#[test]
fn or_binds_tighter_than_and() {
    let parsed =
        parse_predicate("target.has_tag('a') && target.has_tag('b') || target.has_tag('c')")
            .expect("and-or chain parses");
    let root = parsed
        .nodes
        .iter()
        .find(|node| node.node_id == parsed.root_node_id)
        .expect("root node");
    let Op::And(and) = &root.op else {
        panic!("root must be And, got {:?}", root.op);
    };
    assert_eq!(and.operands.len(), 2);
    let second = parsed
        .nodes
        .iter()
        .find(|node| node.node_id == and.operands[1])
        .expect("second operand");
    assert!(
        matches!(second.op, Op::Or(_)),
        "second And operand must be Or(b, c), got {:?}",
        second.op
    );

    assert!(
        parse_predicate("(target.has_tag('a') && target.has_tag('b')) || target.has_tag('c')")
            .is_err(),
        "boolean terms must not be parenthesisable"
    );
}

// ── BoolLiteral ───────────────────────────────────────────────────────────────

#[test]
fn validates_bool_literal_expression() {
    let expr = Expression {
        kind: ExpressionKind::Predicate,
        root_node_id: 1,
        nodes: vec![node(
            1,
            ValueType::Bool,
            Op::BoolLiteral(spellkave_dsl::BoolLiteral { value: true }),
        )],
    };
    let canonical = expr.canonical_json().expect("canonical json");
    assert!(canonical.contains("BoolLiteral"));
    assert!(validate_expression_for_insert(&expr, ExpressionKind::Predicate).is_ok());
}

// ── TextLiteral ───────────────────────────────────────────────────────────────

#[test]
fn validates_text_literal_expression() {
    let expr = Expression {
        kind: ExpressionKind::Outcome,
        root_node_id: 1,
        nodes: vec![node(
            1,
            ValueType::Text,
            Op::TextLiteral(spellkave_dsl::TextLiteral {
                value: "hello".into(),
            }),
        )],
    };
    let canonical = expr.canonical_json().expect("canonical json");
    assert!(canonical.contains("TextLiteral"));
    assert!(validate_expression_for_insert(&expr, ExpressionKind::Outcome).is_ok());
}

// ── Full CompareOp set ────────────────────────────────────────────────────────

#[test]
fn parses_all_compare_ops() {
    for (op_str, expected_tag) in [
        (">=", "Gte"),
        (">", "Gt"),
        ("<=", "Lte"),
        ("<", "Lt"),
        ("==", "Eq"),
        ("!=", "Ne"),
    ] {
        let source = format!("actor.stat(\"str\") {op_str} 10");
        let parsed = parse_predicate(&source)
            .unwrap_or_else(|_| panic!("compare op `{op_str}` should parse"));
        let canonical = parsed.canonical_json().expect("canonical json");
        assert!(
            canonical.contains(expected_tag),
            "op `{op_str}` — expected tag `{expected_tag}` in: {canonical}"
        );
    }
}

// ── Full ArithmeticOp set ─────────────────────────────────────────────────────

#[test]
fn validates_all_arithmetic_ops() {
    for (op, tag) in [
        (spellkave_dsl::ArithmeticOp::Add, "Add"),
        (spellkave_dsl::ArithmeticOp::Sub, "Sub"),
        (spellkave_dsl::ArithmeticOp::Mul, "Mul"),
        (spellkave_dsl::ArithmeticOp::Div, "Div"),
        (spellkave_dsl::ArithmeticOp::Max, "Max"),
        (spellkave_dsl::ArithmeticOp::Min, "Min"),
    ] {
        let expr = Expression {
            kind: ExpressionKind::Predicate,
            root_node_id: 3,
            nodes: vec![
                node(
                    1,
                    ValueType::Int,
                    Op::IntLiteral(spellkave_dsl::IntLiteral { value: 4 }),
                ),
                node(
                    2,
                    ValueType::Int,
                    Op::IntLiteral(spellkave_dsl::IntLiteral { value: 2 }),
                ),
                node(
                    3,
                    ValueType::Int,
                    Op::Arithmetic(spellkave_dsl::Arithmetic {
                        op,
                        left: 1,
                        right: 2,
                    }),
                ),
            ],
        };
        let canonical = expr.canonical_json().expect("canonical json");
        assert!(
            canonical.contains(tag),
            "ArithmeticOp::{tag} missing in: {canonical}"
        );
    }
}

// ── min() parser ──────────────────────────────────────────────────────────────

#[test]
fn parses_min_function() {
    let source = "min(actor.stat(\"str\"), 5) >= 1";
    let parsed = parse_predicate(source).expect("min should parse");
    let canonical = parsed.canonical_json().expect("canonical json");
    assert!(canonical.contains("\"Min\""));
}

#[test]
fn parses_roll_table_as_int_expression() {
    let source = "min(roll_table('core/d20'), roll_table(\"core/d20\")) >= 1";
    let parsed = parse_predicate(source).expect("roll_table int expression should parse");
    let canonical = parsed.canonical_json().expect("canonical json");
    assert!(canonical.contains("RollTable"));
    assert!(canonical.contains("core/d20"));
    assert!(canonical.contains("\"Int\""));
}

#[test]
fn parses_let_bindings() {
    let source = "let a = roll_table('core/d20'); let b = roll_table('core/d20'); min(a, b) >= 1";
    let parsed = parse_predicate(source).expect("let-bound roll table expression should parse");
    let canonical = parsed.canonical_json().expect("canonical json");
    assert!(canonical.contains("Let"));
    assert!(canonical.contains("\"name\":\"a\""));
    assert!(canonical.contains("\"name\":\"b\""));
    assert!(canonical.contains("\"Binding\""));
}

#[test]
fn parses_event_binding_refs() {
    let source = "$event.kind == 'Layer2.ApplyConsequence' && $event.detail.item_id == 42";
    let parsed = parse_predicate(source).expect("event binding predicate should parse");
    let canonical = parsed.canonical_json().expect("canonical json");

    assert!(canonical.contains("\"Event\""));
    assert!(canonical.contains("\"Kind\""));
    assert!(canonical.contains("item_id"));
    assert!(canonical.contains("\"ReadText\""));
    assert!(canonical.contains("\"ReadInt\""));
}

#[test]
fn validate_rejects_let_shadowing() {
    let expr = Expression {
        kind: ExpressionKind::Predicate,
        root_node_id: 5,
        nodes: vec![
            node(
                1,
                ValueType::Int,
                Op::IntLiteral(spellkave_dsl::IntLiteral { value: 1 }),
            ),
            node(
                2,
                ValueType::Int,
                Op::IntLiteral(spellkave_dsl::IntLiteral { value: 2 }),
            ),
            node(
                3,
                ValueType::Bool,
                Op::Compare(spellkave_dsl::Compare {
                    op: spellkave_dsl::CompareOp::Eq,
                    left: 2,
                    right: 1,
                }),
            ),
            node(
                4,
                ValueType::Bool,
                Op::Let(spellkave_dsl::Let {
                    name: "a".into(),
                    value: 2,
                    body: 3,
                }),
            ),
            node(
                5,
                ValueType::Bool,
                Op::Let(spellkave_dsl::Let {
                    name: "a".into(),
                    value: 1,
                    body: 4,
                }),
            ),
        ],
    };

    assert!(matches!(
        validate_expression_for_insert(&expr, ExpressionKind::Predicate),
        Err(spellkave_dsl::ValidationError::ShadowedBinding { name, .. }) if name == "a"
    ));
}

// ── Binding subject ───────────────────────────────────────────────────────────

#[test]
fn validates_binding_subject() {
    let expr = Expression {
        kind: ExpressionKind::Predicate,
        root_node_id: 1,
        nodes: vec![node(
            1,
            ValueType::Bool,
            Op::HasCapability(spellkave_dsl::HasCapability {
                subject: SubjectRef::Binding(spellkave_dsl::BindingRef {
                    name: "victim".into(),
                }),
                capability_id: "core/walk".into(),
            }),
        )],
    };
    let canonical = expr.canonical_json().expect("canonical json");
    assert!(canonical.contains("Binding"));
    assert!(canonical.contains("victim"));
    assert!(validate_expression_for_insert(&expr, ExpressionKind::Predicate).is_ok());
}

// ── TracePresent ──────────────────────────────────────────────────────────────

#[test]
fn parses_trace_predicate() {
    let source = r#"actor.trace("core/exhausted") == true"#;
    let parsed = parse_predicate(source).expect("trace predicate should parse");
    let canonical = parsed.canonical_json().expect("canonical json");
    assert!(canonical.contains("TracePresent"));
}

// ── Outcome-side ops ──────────────────────────────────────────────────────────

#[test]
fn validates_outcome_side_ops() {
    let expr = Expression {
        kind: ExpressionKind::Outcome,
        root_node_id: 6,
        nodes: vec![
            node(
                1,
                ValueType::Int,
                Op::IntLiteral(spellkave_dsl::IntLiteral { value: -3 }),
            ),
            node(
                2,
                ValueType::Outcome,
                Op::ModifyResource(spellkave_dsl::ModifyResource {
                    subject: SubjectRef::Target,
                    resource_definition_id: "core/str".into(),
                    delta_node: 1,
                }),
            ),
            node(
                3,
                ValueType::Outcome,
                Op::InvokeOutcomeGraph(spellkave_dsl::InvokeOutcomeGraph {
                    outcome_graph_id: "core/cairn_critical".into(),
                }),
            ),
            node(
                4,
                ValueType::Outcome,
                Op::RollTable(spellkave_dsl::RollTable {
                    table_id_node: 7,
                    draw_node: 1,
                }),
            ),
            node(
                5,
                ValueType::Outcome,
                Op::ApplyStatus(spellkave_dsl::ApplyStatus {
                    subject: SubjectRef::Actor,
                    condition_definition_id: "core/stunned".into(),
                }),
            ),
            node(
                6,
                ValueType::Outcome,
                Op::OutcomeSequence(spellkave_dsl::OutcomeSequence {
                    steps: vec![2, 3, 4, 5],
                }),
            ),
            node(
                7,
                ValueType::Text,
                Op::TextLiteral(spellkave_dsl::TextLiteral {
                    value: "core/cairn_scars".into(),
                }),
            ),
        ],
    };
    let canonical = expr.canonical_json().expect("canonical json");
    assert!(canonical.contains("ModifyResource"));
    assert!(canonical.contains("InvokeOutcomeGraph"));
    assert!(canonical.contains("RollTable"));
    assert!(canonical.contains("ApplyStatus"));
    assert!(canonical.contains("OutcomeSequence"));
    assert!(validate_expression_for_insert(&expr, ExpressionKind::Outcome).is_ok());
}

#[test]
fn validates_emit_trace_and_arbitrate() {
    let expr = Expression {
        kind: ExpressionKind::Outcome,
        root_node_id: 3,
        nodes: vec![
            node(
                1,
                ValueType::Outcome,
                Op::EmitTrace(spellkave_dsl::EmitTrace {
                    subject: SubjectRef::Actor,
                    trace_kind_id: "core/exhausted".into(),
                }),
            ),
            node(
                2,
                ValueType::Outcome,
                Op::InvokeOutcomeGraph(spellkave_dsl::InvokeOutcomeGraph {
                    outcome_graph_id: "core/recovery".into(),
                }),
            ),
            node(
                3,
                ValueType::Outcome,
                Op::Arbitrate(spellkave_dsl::Arbitrate {
                    request_shape_id: "core/warden_choice".into(),
                    context_nodes: vec![1],
                    allowed_outcome_nodes: vec![2],
                }),
            ),
        ],
    };
    let canonical = expr.canonical_json().expect("canonical json");
    assert!(canonical.contains("EmitTrace"));
    assert!(canonical.contains("Arbitrate"));
    assert!(validate_expression_for_insert(&expr, ExpressionKind::Outcome).is_ok());
}

// ── validate_expression_for_insert structural checks ─────────────────────────

#[test]
fn validate_rejects_kind_mismatch() {
    let expr = Expression {
        kind: ExpressionKind::Outcome,
        root_node_id: 1,
        nodes: vec![node(
            1,
            ValueType::Outcome,
            Op::InvokeOutcomeGraph(spellkave_dsl::InvokeOutcomeGraph {
                outcome_graph_id: "core/test".into(),
            }),
        )],
    };
    assert!(matches!(
        validate_expression_for_insert(&expr, ExpressionKind::Predicate),
        Err(spellkave_dsl::ValidationError::KindMismatch { .. })
    ));
}

#[test]
fn validate_rejects_missing_root() {
    let expr = Expression {
        kind: ExpressionKind::Predicate,
        root_node_id: 99,
        nodes: vec![node(
            1,
            ValueType::Bool,
            Op::BoolLiteral(spellkave_dsl::BoolLiteral { value: true }),
        )],
    };
    assert!(matches!(
        validate_expression_for_insert(&expr, ExpressionKind::Predicate),
        Err(spellkave_dsl::ValidationError::RootMissing { .. })
    ));
}

#[test]
fn validate_rejects_duplicate_node_id() {
    let expr = Expression {
        kind: ExpressionKind::Predicate,
        root_node_id: 1,
        nodes: vec![
            node(
                1,
                ValueType::Bool,
                Op::BoolLiteral(spellkave_dsl::BoolLiteral { value: true }),
            ),
            node(
                1,
                ValueType::Bool,
                Op::BoolLiteral(spellkave_dsl::BoolLiteral { value: false }),
            ),
        ],
    };
    assert!(matches!(
        validate_expression_for_insert(&expr, ExpressionKind::Predicate),
        Err(spellkave_dsl::ValidationError::DuplicateNodeId { .. })
    ));
}

#[test]
fn validate_rejects_missing_child() {
    let expr = Expression {
        kind: ExpressionKind::Predicate,
        root_node_id: 1,
        nodes: vec![node(
            1,
            ValueType::Bool,
            Op::Not(spellkave_dsl::UnaryNode { operand: 99 }),
        )],
    };
    assert!(matches!(
        validate_expression_for_insert(&expr, ExpressionKind::Predicate),
        Err(spellkave_dsl::ValidationError::MissingChild { .. })
    ));
}

// ── Spatial reference validation ──────────────────────────────────────────────

#[test]
fn spatial_reference_validation_accepts_persistent_address_shapes() {
    assert!(
        validate_spatial_reference_for_insert(&SpatialReference::PlaceNode(PlaceNodeRef {
            node_id: "hawthorn/gate".into(),
        }))
        .is_ok()
    );
    assert!(
        validate_spatial_reference_for_insert(&SpatialReference::PlaceEdge(PlaceEdgeRef {
            edge_id: 42,
        }))
        .is_ok()
    );
    assert!(
        validate_spatial_reference_for_insert(&SpatialReference::LocationDefinition(
            LocationDefinitionRef {
                location_def_id: "hawthorn_hollow".into(),
            }
        ))
        .is_ok()
    );
    assert!(
        validate_spatial_reference_for_insert(&SpatialReference::H3Cell(H3CellRef {
            cell: 622236750593089535,
        }))
        .is_ok()
    );
    assert!(
        validate_spatial_reference_for_insert(&SpatialReference::Tile(TileRef {
            substrate_id: "outdoor".into(),
            coordinate_kind_id: "h3_cell".into(),
            value: "622236750593089535".into(),
        }))
        .is_ok()
    );
    assert!(
        validate_spatial_reference_for_insert(&SpatialReference::Tile(TileRef {
            substrate_id: "indoor".into(),
            coordinate_kind_id: "graph_local".into(),
            value: "hawthorn/gate".into(),
        }))
        .is_ok()
    );
}

#[test]
fn spatial_reference_validation_rejects_invalid_and_tactical_addresses() {
    assert!(
        validate_spatial_reference_for_insert(&SpatialReference::PlaceNode(PlaceNodeRef {
            node_id: "hawthorn/gate/extra".into(),
        }))
        .is_ok(),
        "hierarchical place_graph_id must parse on the last slash"
    );
    assert!(matches!(
        validate_spatial_reference_for_insert(&SpatialReference::PlaceNode(PlaceNodeRef {
            node_id: "no_slash".into(),
        })),
        Err(SpatialReferenceValidationError::InvalidPlaceNodeId { .. })
    ));
    assert!(matches!(
        validate_spatial_reference_for_insert(&SpatialReference::PlaceEdge(PlaceEdgeRef {
            edge_id: 0,
        })),
        Err(SpatialReferenceValidationError::InvalidPlaceEdgeId { .. })
    ));
    assert!(matches!(
        validate_spatial_reference_for_insert(&SpatialReference::H3Cell(H3CellRef { cell: 0 })),
        Err(SpatialReferenceValidationError::InvalidH3Cell { .. })
    ));
    assert!(matches!(
        validate_spatial_reference_for_insert(&SpatialReference::Tile(TileRef {
            substrate_id: "tactical".into(),
            coordinate_kind_id: "tactical_grid".into(),
            value: "combat/3/4".into(),
        })),
        Err(SpatialReferenceValidationError::TacticalGridRejected { .. })
    ));
}

// ── CR-17 / LR-17: standalone dice int term, shared with roll_under ───────────

/// The `(count, sides)` of the first `Op::Roll` node in an expression.
fn first_roll_dice(expr: &Expression) -> Option<(u32, u32)> {
    expr.nodes.iter().find_map(|n| match &n.op {
        Op::Roll(roll) => Some((roll.dice.count, roll.dice.sides)),
        _ => None,
    })
}

/// Evaluate an int expression against empty facts with a fixed draw stream.
fn eval_int(source: &str, draws: Vec<u32>) -> i64 {
    let expr = parse_int_expression(source).expect("int expression should parse");
    let facts = EvaluationContext::default();
    let mut rng = DeterministicRng::new(draws);
    evaluate_int_expression(&expr, &facts, &mut rng, EvaluationLimits::default())
        .expect("int expression should evaluate")
}

/// `parse_int_expression` accepts a bare dice atom (`dN`) as an Outcome-kind
/// int expression — the shape ADR-0037 initiative needs (CR-17 / LR-17).
#[test]
fn parse_int_expression_accepts_bare_die() {
    let expr = parse_int_expression("d20").expect("d20 should parse");
    assert_eq!(expr.kind, ExpressionKind::Outcome);
    assert_eq!(first_roll_dice(&expr), Some((1, 20)));
    // Round-trips through the Outcome validation `evaluate_int_expression` runs.
    validate_expression_for_insert(&expr, ExpressionKind::Outcome).expect("validates as Outcome");
}

/// `XdN` carries the count through to the AST (no longer discarded — LR-17).
#[test]
fn parse_int_expression_carries_dice_count() {
    assert_eq!(
        first_roll_dice(&parse_int_expression("2d6").unwrap()),
        Some((2, 6))
    );
    assert_eq!(
        first_roll_dice(&parse_int_expression("3d4").unwrap()),
        Some((3, 4))
    );
    // Arithmetic over a dice atom still parses (e.g. `d20 - 2`).
    let expr = parse_int_expression("d20 - 2").expect("dice arithmetic should parse");
    let canonical = expr.canonical_json().expect("canonical json");
    assert!(canonical.contains("\"Sub\""));
    assert_eq!(first_roll_dice(&expr), Some((1, 20)));
}

/// A malformed dice token does not silently become a single die: `2d` (no
/// sides) leaves trailing input, and an empty source is rejected.
#[test]
fn parse_int_expression_rejects_malformed_dice() {
    assert_eq!(
        parse_int_expression("2d")
            .expect_err("2d has no sides")
            .diagnostics[0]
            .code,
        "MF-DSL-TRAILING"
    );
    assert!(parse_int_expression("").is_err());
}

/// `roll_under(d20, …)` still produces the byte-identical inner `Dice { 1, 20 }`
/// after the shared-grammar refactor (CR-17 / canon-warden C-1).
#[test]
fn roll_under_d20_inner_dice_is_unchanged() {
    let expr = parse_predicate(r#"roll_under(d20, actor.stat("dex"))"#).expect("roll_under parses");
    assert_eq!(first_roll_dice(&expr), Some((1, 20)));
    // The shared grammar now also accepts a multiplier inside roll_under.
    let multi = parse_predicate("roll_under(2d6, 5)").expect("roll_under(2d6) parses");
    assert_eq!(first_roll_dice(&multi), Some((2, 6)));
}

/// The evaluator HONORS the dice count: `2d6` sums two draws, never one
/// (LR-17's silent-single-die lie, proven dead). `roll = draw % sides + 1`.
#[test]
fn evaluate_int_expression_honors_dice_count() {
    // One die, one draw → 1; two dice, two zero-draws → 1 + 1 = 2.
    assert_eq!(eval_int("d6", vec![0, 0, 0]), 1);
    assert_eq!(eval_int("2d6", vec![0, 0, 0]), 2);
    // Maxed draws: 2d6 = 6 + 6 = 12; 3d4 = 4 + 4 + 4 = 12.
    assert_eq!(eval_int("2d6", vec![5, 5, 5]), 12);
    assert_eq!(eval_int("3d4", vec![3, 3, 3]), 12);
    // d20 spans 1..=20 with the expected modulo mapping.
    assert_eq!(eval_int("d20", vec![19]), 20);
    assert_eq!(eval_int("d20", vec![20]), 1);
}

// ── entity-ref binding classifier (CR-18 — shared by apply + validator) ─────────

#[test]
fn entity_ref_binding_surfaces_only_knows_entity_paths() {
    // KnowsEntity's argument is an entity ref → the binding name is surfaced
    // (apply resolves it to a numeric id; the validator checks it resolves).
    let knows =
        parse_predicate("actor.knows_entity($bindings.castle)").expect("knows_entity should parse");
    let names: Vec<&str> = knows
        .nodes
        .iter()
        .filter_map(|n| n.op.entity_ref_binding())
        .collect();
    assert_eq!(names, vec!["castle"]);

    // has_item's argument is a def-id string compared as-is, NOT an entity ref —
    // it must never be classified as one (else the apply would corrupt it).
    let has_item = parse_predicate(r#"actor.has_item($bindings.required_key)"#)
        .expect("has_item should parse");
    assert!(
        has_item
            .nodes
            .iter()
            .all(|n| n.op.entity_ref_binding().is_none()),
        "has_item bindings are def-id strings, not entity refs"
    );
}
