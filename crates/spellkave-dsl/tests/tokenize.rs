// DSL tools receive token spans that reproduce every byte of authored source.
use spellkave_dsl::{tokenize, TokenKind};

fn reconstruct(source: &str) -> String {
    let tokens = tokenize(source).unwrap_or_else(|err| {
        panic!("tokenize failed on {source:?}: {err:?}");
    });
    tokens
        .iter()
        .map(|token| {
            assert!(
                !matches!(token.kind, TokenKind::Eof),
                "tokenize must omit Eof"
            );
            &source[token.span.start..token.span.end]
        })
        .collect()
}

#[test]
fn tokenize_round_trips_whitespace_and_operators() {
    let source = "a &&\n b || c";
    assert_eq!(reconstruct(source), source);
    let tokens = tokenize(source).expect("lex");
    assert!(tokens
        .iter()
        .any(|t| matches!(t.kind, TokenKind::Whitespace)));
    assert!(tokens.iter().any(|t| matches!(t.kind, TokenKind::AndAnd)));
}

#[test]
fn tokenize_has_no_dice_kind() {
    let tokens = tokenize("2d6 + d20").expect("lex");
    assert!(tokens
        .iter()
        .any(|t| matches!(t.kind, TokenKind::Number(2))));
    assert!(tokens
        .iter()
        .any(|t| matches!(&t.kind, TokenKind::Ident(name) if name == "d6")));
    assert!(tokens
        .iter()
        .any(|t| matches!(&t.kind, TokenKind::Ident(name) if name == "d20")));
    assert!(!tokens
        .iter()
        .any(|t| format!("{:?}", t.kind).to_lowercase().contains("dice")));
}

#[test]
fn tokenize_empty_is_empty() {
    assert!(tokenize("").expect("lex").is_empty());
    assert_eq!(reconstruct(""), "");
}
