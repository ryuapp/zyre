use crate::lexer::{Token, tokenize};

fn toks(src: &str) -> Vec<Token> {
    tokenize(src).into_iter().map(|(t, _)| t).collect()
}

#[test]
fn test_lex_comment_skipped() {
    let t = toks("// hello world\n42");
    assert!(t.contains(&Token::Int(42)));
    assert!(!t.iter().any(|t| *t == Token::Ident("hello".into())));
}

#[test]
fn test_lex_comment_inline() {
    let t = toks("42 // comment");
    assert!(t.contains(&Token::Int(42)));
}

#[test]
fn test_lex_slash() {
    let t = toks("10 / 2");
    assert!(t.contains(&Token::Slash));
}

#[test]
fn test_lex_not_eq() {
    let t = toks("a != b");
    assert!(t.contains(&Token::NotEq));
}

#[test]
fn test_lex_bang() {
    let t = toks("!T");
    assert!(t.contains(&Token::Bang));
}

#[test]
fn test_lex_lt_lteq() {
    assert!(toks("a < b").contains(&Token::Lt));
    assert!(toks("a <= b").contains(&Token::LtEq));
}

#[test]
fn test_lex_gt_gteq() {
    assert!(toks("a > b").contains(&Token::Gt));
    assert!(toks("a >= b").contains(&Token::GtEq));
}

#[test]
fn test_lex_and() {
    assert!(toks("a and b").contains(&Token::And));
}

#[test]
fn test_lex_or() {
    assert!(toks("a or b").contains(&Token::Or));
}

#[test]
fn test_lex_minus() {
    assert!(toks("a - b").contains(&Token::Minus));
}

#[test]
fn test_lex_unknown_char_skipped() {
    let t = toks("1 @ 2");
    assert!(t.contains(&Token::Int(1)));
    assert!(t.contains(&Token::Int(2)));
}
