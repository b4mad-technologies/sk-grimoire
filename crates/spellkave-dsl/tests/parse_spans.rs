use spellkave_dsl::{parse_int_expression, parse_predicate};

#[test]
fn parse_predicate_root_span_covers_the_source() {
    let source = "1 == 1";
    let expr = parse_predicate(source).expect("parse");
    let root = expr.node(expr.root_node_id).expect("root");
    assert_eq!(root.span.start, 0);
    assert_eq!(root.span.end, source.len());
    assert!(expr.nodes.iter().all(|node| !node.span.is_empty()));
}

#[test]
fn parse_int_expression_root_span_covers_the_source() {
    let source = "2d6 + 1";
    let expr = parse_int_expression(source).expect("parse");
    let root = expr.node(expr.root_node_id).expect("root");
    assert_eq!(root.span.start, 0);
    assert_eq!(root.span.end, source.len());
}

#[test]
fn compile_star_behaviour_still_parses_the_same_tree_shape() {
    let source = "actor.stat('srd/ac') >= 10";
    let expr = parse_predicate(source).expect("parse");
    assert!(expr.canonical_json().expect("json").contains("ReadInt"));
    let root = expr.node(expr.root_node_id).expect("root");
    assert_eq!(root.span.end, source.len());
}
