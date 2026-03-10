#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    // Keywords
    Const,
    Let,
    Fn,
    Export,
    Import,
    Return,
    If,
    While,
    Break,
    Continue,
    Switch,
    Catch,
    Else,
    Then,
    // Type keywords
    Void,
    // Literals
    Ident(String),
    Str(String),
    Int(i64),
    Float(f64),
    True,
    False,
    // Operators
    Eq,       // =
    EqEq,     // ==
    NotEq,    // !=
    Lt,       // <
    Gt,       // >
    LtEq,     // <=
    GtEq,     // >=
    Plus,     // +
    Minus,    // -
    Star,     // *
    Slash,    // /
    Percent,  // %
    And,      // &&
    Or,       // ||
    Bang,     // !
    Question, // ?
    FatArrow, // =>
    // Delimiters
    LParen,   // (
    RParen,   // )
    LBrace,   // {
    RBrace,   // }
    LBracket, // [
    RBracket, // ]
    Colon,    // :
    Semi,     // ;
    AutoSemi, // ; automatically inserted from newline
    Dot,      // .
    Comma,    // ,
    EOF,
}

/// Byte offset range (start, end) within the source string
pub type Span = (usize, usize);

/// Tokens that trigger automatic `;` insertion before a newline (Go-style)
fn can_end_stmt(tok: &Token) -> bool {
    matches!(
        tok,
        Token::Ident(_)
            | Token::Int(_)
            | Token::Float(_)
            | Token::Str(_)
            | Token::True
            | Token::False
            | Token::RParen
            | Token::RBracket
            | Token::RBrace
            | Token::Question
            | Token::Break
            | Token::Continue
            | Token::Return
    )
}

/// Tokens that suppress automatic `;` inserted just before them
fn no_semi_before(tok: &Token) -> bool {
    matches!(
        tok,
        Token::RParen
            | Token::RBracket
            | Token::RBrace
            | Token::Comma
            | Token::Colon
            | Token::Dot
            | Token::EOF
    )
}

pub fn tokenize(source: &str) -> Vec<(Token, Span)> {
    let tokens = tokenize_raw(source);
    // AutoSemi: drop it if the next token suppresses it, otherwise convert to Semi
    // Explicit Semi tokens are kept as-is
    let mut result = Vec::with_capacity(tokens.len());
    let mut i = 0;
    while i < tokens.len() {
        if tokens[i].0 == Token::AutoSemi {
            let next = tokens.get(i + 1).map(|(t, _)| t).unwrap_or(&Token::EOF);
            if no_semi_before(next) {
                i += 1;
                continue;
            }
            result.push((Token::Semi, tokens[i].1));
        } else {
            result.push(tokens[i].clone());
        }
        i += 1;
    }
    result
}

fn tokenize_raw(source: &str) -> Vec<(Token, Span)> {
    let mut tokens = Vec::new();
    let mut chars = source.char_indices().peekable();

    macro_rules! end_pos {
        () => {
            chars.peek().map(|&(p, _)| p).unwrap_or(source.len())
        };
    }

    while let Some(&(pos, ch)) = chars.peek() {
        match ch {
            '\r' => {
                chars.next();
            }
            '\n' => {
                chars.next();
                // Insert AutoSemi if the previous token can end a statement
                if tokens.last().is_some_and(|(t, _)| can_end_stmt(t)) {
                    tokens.push((Token::AutoSemi, (pos, pos + 1)));
                }
            }
            ' ' | '\t' => {
                chars.next();
            }
            '/' => {
                chars.next();
                if chars.peek().map(|&(_, c)| c) == Some('/') {
                    while let Some(&(_, c)) = chars.peek() {
                        chars.next();
                        if c == '\n' {
                            break;
                        }
                    }
                } else {
                    tokens.push((Token::Slash, (pos, end_pos!())));
                }
            }
            '=' => {
                chars.next();
                if chars.peek().map(|&(_, c)| c) == Some('=') {
                    chars.next();
                    tokens.push((Token::EqEq, (pos, end_pos!())));
                } else if chars.peek().map(|&(_, c)| c) == Some('>') {
                    chars.next();
                    tokens.push((Token::FatArrow, (pos, end_pos!())));
                } else {
                    tokens.push((Token::Eq, (pos, end_pos!())));
                }
            }
            '!' => {
                chars.next();
                if chars.peek().map(|&(_, c)| c) == Some('=') {
                    chars.next();
                    tokens.push((Token::NotEq, (pos, end_pos!())));
                } else {
                    tokens.push((Token::Bang, (pos, end_pos!())));
                }
            }
            '<' => {
                chars.next();
                if chars.peek().map(|&(_, c)| c) == Some('=') {
                    chars.next();
                    tokens.push((Token::LtEq, (pos, end_pos!())));
                } else {
                    tokens.push((Token::Lt, (pos, end_pos!())));
                }
            }
            '>' => {
                chars.next();
                if chars.peek().map(|&(_, c)| c) == Some('=') {
                    chars.next();
                    tokens.push((Token::GtEq, (pos, end_pos!())));
                } else {
                    tokens.push((Token::Gt, (pos, end_pos!())));
                }
            }
            '+' => {
                chars.next();
                tokens.push((Token::Plus, (pos, end_pos!())));
            }
            '-' => {
                chars.next();
                tokens.push((Token::Minus, (pos, end_pos!())));
            }
            '*' => {
                chars.next();
                tokens.push((Token::Star, (pos, end_pos!())));
            }
            '%' => {
                chars.next();
                tokens.push((Token::Percent, (pos, end_pos!())));
            }
            '.' => {
                chars.next();
                tokens.push((Token::Dot, (pos, end_pos!())));
            }
            ',' => {
                chars.next();
                tokens.push((Token::Comma, (pos, end_pos!())));
            }
            '(' => {
                chars.next();
                tokens.push((Token::LParen, (pos, end_pos!())));
            }
            ')' => {
                chars.next();
                tokens.push((Token::RParen, (pos, end_pos!())));
            }
            '{' => {
                chars.next();
                tokens.push((Token::LBrace, (pos, end_pos!())));
            }
            '}' => {
                chars.next();
                tokens.push((Token::RBrace, (pos, end_pos!())));
            }
            '[' => {
                chars.next();
                tokens.push((Token::LBracket, (pos, end_pos!())));
            }
            ']' => {
                chars.next();
                tokens.push((Token::RBracket, (pos, end_pos!())));
            }
            ':' => {
                chars.next();
                tokens.push((Token::Colon, (pos, end_pos!())));
            }
            ';' => {
                chars.next();
                tokens.push((Token::Semi, (pos, end_pos!())));
            }
            '?' => {
                chars.next();
                tokens.push((Token::Question, (pos, end_pos!())));
            }
            '"' => {
                chars.next(); // opening "
                let mut s = String::new();
                while let Some(&(_, c)) = chars.peek() {
                    if c == '"' {
                        chars.next(); // closing "
                        break;
                    }
                    s.push(c);
                    chars.next();
                }
                tokens.push((Token::Str(s), (pos, end_pos!())));
            }
            c if c.is_ascii_digit() => {
                let mut num = String::new();
                let mut is_float = false;
                while let Some(&(_, c)) = chars.peek() {
                    if c.is_ascii_digit() {
                        num.push(c);
                        chars.next();
                    } else if c == '.' && !is_float {
                        is_float = true;
                        num.push(c);
                        chars.next();
                    } else {
                        break;
                    }
                }
                let end = end_pos!();
                if is_float {
                    tokens.push((Token::Float(num.parse().unwrap()), (pos, end)));
                } else {
                    tokens.push((Token::Int(num.parse().unwrap()), (pos, end)));
                }
            }
            c if c.is_alphabetic() || c == '_' => {
                let mut ident = String::new();
                while let Some(&(_, c)) = chars.peek() {
                    if c.is_alphanumeric() || c == '_' {
                        ident.push(c);
                        chars.next();
                    } else {
                        break;
                    }
                }
                let end = end_pos!();
                let tok = match ident.as_str() {
                    "const" => Token::Const,
                    "let" => Token::Let,
                    "fn" => Token::Fn,
                    "export" => Token::Export,
                    "import" => Token::Import,
                    "return" => Token::Return,
                    "if" => Token::If,
                    "while" => Token::While,
                    "break" => Token::Break,
                    "continue" => Token::Continue,
                    "switch" => Token::Switch,
                    "catch" => Token::Catch,
                    "else" => Token::Else,
                    "then" => Token::Then,
                    "and" => Token::And,
                    "or" => Token::Or,
                    "void" => Token::Void,
                    "true" => Token::True,
                    "false" => Token::False,
                    _ => Token::Ident(ident),
                };
                tokens.push((tok, (pos, end)));
            }
            _ => {
                chars.next();
            }
        }
    }

    tokens.push((Token::EOF, (source.len(), source.len())));
    tokens
}
