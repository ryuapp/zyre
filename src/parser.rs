use crate::lexer::{Span, Token};

#[derive(Debug, Clone)]
pub struct ParseError {
    pub message: String,
    pub span: Span,
}

fn token_display(tok: &Token) -> String {
    match tok {
        Token::Semi => "`;`".to_string(),
        Token::LParen => "`(`".to_string(),
        Token::RParen => "`)`".to_string(),
        Token::LBrace => "`{`".to_string(),
        Token::RBrace => "`}`".to_string(),
        Token::LBracket => "`[`".to_string(),
        Token::RBracket => "`]`".to_string(),
        Token::Eq => "`=`".to_string(),
        Token::Colon => "`:`".to_string(),
        Token::Comma => "`,`".to_string(),
        Token::FatArrow => "`=>`".to_string(),
        Token::Ident(s) => format!("`{}`", s),
        Token::EOF => "end of file".to_string(),
        tok => format!("`{:?}`", tok),
    }
}

// --- Type expressions ---

#[derive(Debug, Clone)]
pub enum TypeExpr {
    Name(String),              // i32, f64, string, bool, void, user-defined types
    Array(Box<TypeExpr>, u64), // T[N]
    Optional(Box<TypeExpr>),   // ?T
    ErrorUnion(Option<Box<TypeExpr>>, Box<TypeExpr>), // E!T or !T
}

// --- Expressions ---

#[derive(Debug, Clone)]
pub enum ExprKind {
    Import(String),
    Var(String),
    Str(String),
    Int(i64),
    Float(f64),
    Bool(bool),
    MemberAccess {
        obj: Box<Expr>,
        prop: String,
    },
    Call {
        callee: Box<Expr>,
        args: Vec<Expr>,
    },
    BinOp {
        op: BinOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },
    UnOp {
        op: UnOp,
        expr: Box<Expr>,
    },
    Propagate(Box<Expr>), // expr? (error propagation)
    Catch {
        expr: Box<Expr>,
        err_name: String,
        body: Vec<Stmt>,
    },
    Switch {
        expr: Box<Expr>,
        arms: Vec<SwitchArm>,
    },
    ArrayLiteral(Vec<Expr>), // [1, 2, 3] array literal
    Index {
        obj: Box<Expr>,
        idx: Box<Expr>,
    }, // arr[i] index expression
    If {
        cond: Box<Expr>,
        then: Box<Expr>,
        else_: Box<Expr>,
    },
}

#[derive(Debug, Clone)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Eq,
    NotEq,
    Lt,
    Gt,
    LtEq,
    GtEq,
    And,
    Or,
}

#[derive(Debug, Clone)]
pub enum UnOp {
    Neg,
    Not,
}

#[derive(Debug, Clone)]
pub struct SwitchArm {
    pub pattern: SwitchPattern,
    pub body: SwitchBody,
}

#[derive(Debug, Clone)]
pub enum SwitchPattern {
    Ident(String),
    Int(i64),
    Bool(bool),
    Else,
}

#[derive(Debug, Clone)]
pub enum SwitchBody {
    Expr(Expr),
    Block(Vec<Stmt>),
}

// --- Statements ---

#[derive(Debug, Clone)]
pub enum StmtKind {
    ConstDecl {
        name: String,
        ty: Option<TypeExpr>,
        value: Expr,
    },
    LetDecl {
        name: String,
        ty: Option<TypeExpr>,
        value: Expr,
    },
    Return(Option<Expr>),
    If {
        cond: Expr,
        body: Vec<Stmt>,
        else_body: Option<Vec<Stmt>>,
    },
    While {
        cond: Expr,
        body: Vec<Stmt>,
    },
    Break,
    Continue,
    ExprStmt(Expr),
}

#[derive(Debug, Clone)]
pub struct Stmt {
    pub kind: StmtKind,
    pub span: Span,
}

// --- Top-level items ---

#[derive(Debug, Clone)]
pub struct StructField {
    pub name: String,
    pub ty: TypeExpr,
}

#[derive(Debug, Clone)]
pub struct FnDecl {
    pub name: String,
    pub params: Vec<(String, TypeExpr)>,
    pub ret: TypeExpr,
    pub body: Vec<Stmt>,
    pub exported: bool,
}

#[derive(Debug, Clone)]
pub enum TopLevel {
    ConstDecl {
        name: String,
        name_span: Span,
        value: Expr,
        exported: bool,
    },
    FnDecl(FnDecl),
    StructDecl {
        name: String,
        fields: Vec<StructField>,
        exported: bool,
    },
    EnumDecl {
        name: String,
        variants: Vec<String>,
        exported: bool,
    },
    Stmt(Stmt), // top-level statement (implicitly placed in main)
}

pub type Program = Vec<TopLevel>;

// --- Parser ---

pub struct Parser {
    tokens: Vec<(Token, Span)>,
    pos: usize,
    pub errors: Vec<ParseError>,
}

impl Parser {
    pub fn new(tokens: Vec<(Token, Span)>) -> Self {
        Parser {
            tokens,
            pos: 0,
            errors: Vec::new(),
        }
    }

    fn error(&mut self, msg: String) {
        self.errors.push(ParseError {
            message: msg,
            span: self.peek_span(),
        });
    }

    fn peek(&self) -> &Token {
        &self.tokens[self.pos].0
    }

    fn peek_span(&self) -> Span {
        self.tokens[self.pos].1
    }

    fn prev_end(&self) -> usize {
        if self.pos > 0 {
            self.tokens[self.pos - 1].1.1
        } else {
            0
        }
    }

    fn advance(&mut self) -> Token {
        let (tok, _) = self.tokens[self.pos].clone();
        if self.pos + 1 < self.tokens.len() {
            self.pos += 1;
        }
        tok
    }

    fn expect_ident_spanned(&mut self) -> (String, Span) {
        let span = self.tokens[self.pos].1;
        match self.peek().clone() {
            Token::Ident(s) => {
                self.advance();
                (s, span)
            }
            tok => {
                self.error(format!("Expected identifier, got {}", token_display(&tok)));
                ("<error>".to_string(), span)
            }
        }
    }

    fn expect_ident(&mut self) -> String {
        self.expect_ident_spanned().0
    }

    fn expect(&mut self, expected: Token) {
        if self.peek() == &expected {
            self.advance();
        } else {
            let got = self.peek().clone();
            self.error(format!(
                "Expected {}, got {}",
                token_display(&expected),
                token_display(&got)
            ));
        }
    }

    /// Consume `;` or AutoSemi if present; does not error if absent
    fn eat_semi(&mut self) {
        if matches!(self.peek(), Token::Semi | Token::AutoSemi) {
            self.advance();
        }
    }

    // --- Type parsing ---

    fn parse_type(&mut self) -> TypeExpr {
        match self.peek().clone() {
            Token::Question => {
                self.advance();
                let inner = self.parse_type();
                TypeExpr::Optional(Box::new(inner))
            }
            Token::Bang => {
                self.advance();
                let inner = self.parse_type();
                TypeExpr::ErrorUnion(None, Box::new(inner))
            }
            Token::Ident(name) => {
                self.advance();
                // Check for T[N] form
                if self.peek() == &Token::LBracket {
                    self.advance();
                    let n = match self.peek().clone() {
                        Token::Int(n) => {
                            self.advance();
                            n as u64
                        }
                        tok => {
                            self.error(format!("Expected array size, got {}", token_display(&tok)));
                            0
                        }
                    };
                    self.expect(Token::RBracket);
                    TypeExpr::Array(Box::new(TypeExpr::Name(name)), n)
                // Check for E!T form
                } else if self.peek() == &Token::Bang {
                    self.advance();
                    let inner = self.parse_type();
                    TypeExpr::ErrorUnion(Some(Box::new(TypeExpr::Name(name))), Box::new(inner))
                } else {
                    TypeExpr::Name(name)
                }
            }
            Token::Void => {
                self.advance();
                TypeExpr::Name("void".to_string())
            }
            tok => {
                self.error(format!("Expected type, got {}", token_display(&tok)));
                TypeExpr::Name("<error>".to_string())
            }
        }
    }

    // --- Program ---

    pub fn parse_program(&mut self) -> Program {
        let mut items = Vec::new();
        while self.peek() != &Token::EOF {
            // Skip stray `;` / AutoSemi at the top level
            while matches!(self.peek(), Token::Semi | Token::AutoSemi) {
                self.advance();
            }
            if self.peek() == &Token::EOF {
                break;
            }
            items.push(self.parse_toplevel());
        }
        items
    }

    fn parse_toplevel(&mut self) -> TopLevel {
        let exported = if self.peek() == &Token::Export {
            self.advance();
            true
        } else {
            false
        };

        match self.peek().clone() {
            Token::Const => {
                self.advance();
                let (name, name_span) = self.expect_ident_spanned();
                self.expect(Token::Eq);
                match self.peek().clone() {
                    Token::Import => {
                        let value = self.parse_expr();
                        self.eat_semi();
                        TopLevel::ConstDecl {
                            name,
                            name_span,
                            value,
                            exported,
                        }
                    }
                    Token::Ident(kw) if kw == "struct" => {
                        self.advance();
                        self.expect(Token::LBrace);
                        let mut fields = Vec::new();
                        while self.peek() != &Token::RBrace {
                            let fname = self.expect_ident();
                            self.expect(Token::Colon);
                            let fty = self.parse_type();
                            fields.push(StructField {
                                name: fname,
                                ty: fty,
                            });
                            if self.peek() == &Token::Comma {
                                self.advance();
                            }
                        }
                        self.expect(Token::RBrace);
                        self.eat_semi();
                        TopLevel::StructDecl {
                            name,
                            fields,
                            exported,
                        }
                    }
                    Token::Ident(kw) if kw == "enum" => {
                        self.advance();
                        self.expect(Token::LBrace);
                        let mut variants = Vec::new();
                        while self.peek() != &Token::RBrace {
                            variants.push(self.expect_ident());
                            if self.peek() == &Token::Comma {
                                self.advance();
                            }
                        }
                        self.expect(Token::RBrace);
                        self.eat_semi();
                        TopLevel::EnumDecl {
                            name,
                            variants,
                            exported,
                        }
                    }
                    _ => {
                        let value = self.parse_expr();
                        self.eat_semi();
                        TopLevel::ConstDecl {
                            name,
                            name_span,
                            value,
                            exported,
                        }
                    }
                }
            }
            Token::Fn => {
                self.advance();
                let name = self.expect_ident();
                self.expect(Token::LParen);
                let mut params = Vec::new();
                while self.peek() != &Token::RParen {
                    let pname = self.expect_ident();
                    self.expect(Token::Colon);
                    let pty = self.parse_type();
                    params.push((pname, pty));
                    if self.peek() == &Token::Comma {
                        self.advance();
                    }
                }
                self.expect(Token::RParen);
                self.expect(Token::Colon);
                let ret = self.parse_type();
                self.expect(Token::LBrace);
                let body = self.parse_block();
                self.expect(Token::RBrace);
                TopLevel::FnDecl(FnDecl {
                    name,
                    params,
                    ret,
                    body,
                    exported,
                })
            }
            _ => TopLevel::Stmt(self.parse_stmt()),
        }
    }

    // --- Block parsing ---

    fn parse_block(&mut self) -> Vec<Stmt> {
        let mut stmts = Vec::new();
        while self.peek() != &Token::RBrace && self.peek() != &Token::EOF {
            while matches!(self.peek(), Token::Semi | Token::AutoSemi) {
                self.advance();
            }
            if self.peek() == &Token::RBrace || self.peek() == &Token::EOF {
                break;
            }
            stmts.push(self.parse_stmt());
        }
        stmts
    }

    // --- Statements ---

    fn parse_stmt(&mut self) -> Stmt {
        let span = self.peek_span();
        match self.peek().clone() {
            Token::Const => {
                self.advance();
                let name = self.expect_ident();
                let ty = if self.peek() == &Token::Colon {
                    self.advance();
                    Some(self.parse_type())
                } else {
                    None
                };
                self.expect(Token::Eq);
                let value = self.parse_expr();
                self.eat_semi();
                Stmt {
                    kind: StmtKind::ConstDecl { name, ty, value },
                    span,
                }
            }
            Token::Let => {
                self.advance();
                let name = self.expect_ident();
                let ty = if self.peek() == &Token::Colon {
                    self.advance();
                    Some(self.parse_type())
                } else {
                    None
                };
                self.expect(Token::Eq);
                let value = self.parse_expr();
                self.eat_semi();
                Stmt {
                    kind: StmtKind::LetDecl { name, ty, value },
                    span,
                }
            }
            Token::Return => {
                self.advance();
                // RBrace and EOF terminate return even without a semicolon
                // (AutoSemi before `}` is suppressed by no_semi_before)
                if matches!(
                    self.peek(),
                    Token::Semi | Token::AutoSemi | Token::RBrace | Token::EOF
                ) {
                    self.eat_semi();
                    Stmt {
                        kind: StmtKind::Return(None),
                        span,
                    }
                } else {
                    let expr = self.parse_expr();
                    self.eat_semi();
                    Stmt {
                        kind: StmtKind::Return(Some(expr)),
                        span,
                    }
                }
            }
            Token::If => {
                self.advance();
                let cond = self.parse_expr();
                self.expect(Token::LBrace);
                let body = self.parse_block();
                self.expect(Token::RBrace);
                let else_body = if self.peek() == &Token::Else {
                    self.advance();
                    self.expect(Token::LBrace);
                    let b = self.parse_block();
                    self.expect(Token::RBrace);
                    Some(b)
                } else {
                    None
                };
                Stmt {
                    kind: StmtKind::If {
                        cond,
                        body,
                        else_body,
                    },
                    span,
                }
            }
            Token::While => {
                self.advance();
                let cond = self.parse_expr();
                self.expect(Token::LBrace);
                let body = self.parse_block();
                self.expect(Token::RBrace);
                Stmt {
                    kind: StmtKind::While { cond, body },
                    span,
                }
            }
            Token::Break => {
                self.advance();
                self.eat_semi();
                Stmt {
                    kind: StmtKind::Break,
                    span,
                }
            }
            Token::Continue => {
                self.advance();
                self.eat_semi();
                Stmt {
                    kind: StmtKind::Continue,
                    span,
                }
            }
            _ => {
                let expr = self.parse_expr();
                self.eat_semi();
                Stmt {
                    kind: StmtKind::ExprStmt(expr),
                    span,
                }
            }
        }
    }

    // --- Expressions ---

    fn parse_expr(&mut self) -> Expr {
        self.parse_or()
    }

    fn parse_or(&mut self) -> Expr {
        let mut lhs = self.parse_and();
        while self.peek() == &Token::Or {
            self.advance();
            let rhs = self.parse_and();
            let span = (lhs.span.0, rhs.span.1);
            lhs = Expr {
                kind: ExprKind::BinOp {
                    op: BinOp::Or,
                    lhs: Box::new(lhs),
                    rhs: Box::new(rhs),
                },
                span,
            };
        }
        lhs
    }

    fn parse_and(&mut self) -> Expr {
        let mut lhs = self.parse_cmp();
        while self.peek() == &Token::And {
            self.advance();
            let rhs = self.parse_cmp();
            let span = (lhs.span.0, rhs.span.1);
            lhs = Expr {
                kind: ExprKind::BinOp {
                    op: BinOp::And,
                    lhs: Box::new(lhs),
                    rhs: Box::new(rhs),
                },
                span,
            };
        }
        lhs
    }

    fn parse_cmp(&mut self) -> Expr {
        let lhs = self.parse_add();
        let op = match self.peek() {
            Token::EqEq => BinOp::Eq,
            Token::NotEq => BinOp::NotEq,
            Token::Lt => BinOp::Lt,
            Token::Gt => BinOp::Gt,
            Token::LtEq => BinOp::LtEq,
            Token::GtEq => BinOp::GtEq,
            _ => return lhs,
        };
        self.advance();
        let rhs = self.parse_add();
        let span = (lhs.span.0, rhs.span.1);
        Expr {
            kind: ExprKind::BinOp {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            },
            span,
        }
    }

    fn parse_add(&mut self) -> Expr {
        let mut lhs = self.parse_mul();
        loop {
            let op = match self.peek() {
                Token::Plus => BinOp::Add,
                Token::Minus => BinOp::Sub,
                _ => break,
            };
            self.advance();
            let rhs = self.parse_mul();
            let span = (lhs.span.0, rhs.span.1);
            lhs = Expr {
                kind: ExprKind::BinOp {
                    op,
                    lhs: Box::new(lhs),
                    rhs: Box::new(rhs),
                },
                span,
            };
        }
        lhs
    }

    fn parse_mul(&mut self) -> Expr {
        let mut lhs = self.parse_unary();
        loop {
            let op = match self.peek() {
                Token::Star => BinOp::Mul,
                Token::Slash => BinOp::Div,
                _ => break,
            };
            self.advance();
            let rhs = self.parse_unary();
            let span = (lhs.span.0, rhs.span.1);
            lhs = Expr {
                kind: ExprKind::BinOp {
                    op,
                    lhs: Box::new(lhs),
                    rhs: Box::new(rhs),
                },
                span,
            };
        }
        lhs
    }

    fn parse_unary(&mut self) -> Expr {
        match self.peek().clone() {
            Token::Bang => {
                let span_start = self.peek_span().0;
                self.advance();
                let expr = self.parse_unary();
                let span = (span_start, expr.span.1);
                Expr {
                    kind: ExprKind::UnOp {
                        op: UnOp::Not,
                        expr: Box::new(expr),
                    },
                    span,
                }
            }
            Token::Minus => {
                let span_start = self.peek_span().0;
                self.advance();
                let expr = self.parse_unary();
                let span = (span_start, expr.span.1);
                Expr {
                    kind: ExprKind::UnOp {
                        op: UnOp::Neg,
                        expr: Box::new(expr),
                    },
                    span,
                }
            }
            _ => self.parse_postfix(),
        }
    }

    fn parse_postfix(&mut self) -> Expr {
        let mut expr = self.parse_primary();

        loop {
            match self.peek().clone() {
                Token::Dot => {
                    self.advance();
                    let prop = self.expect_ident();
                    let span = (expr.span.0, self.prev_end());
                    expr = Expr {
                        kind: ExprKind::MemberAccess {
                            obj: Box::new(expr),
                            prop,
                        },
                        span,
                    };
                }
                Token::LParen => {
                    let span_start = expr.span.0;
                    self.advance();
                    let mut args = Vec::new();
                    while self.peek() != &Token::RParen {
                        args.push(self.parse_expr());
                        if self.peek() == &Token::Comma {
                            self.advance();
                        }
                    }
                    self.expect(Token::RParen);
                    let span = (span_start, self.prev_end());
                    expr = Expr {
                        kind: ExprKind::Call {
                            callee: Box::new(expr),
                            args,
                        },
                        span,
                    };
                }
                Token::LBracket => {
                    let span_start = expr.span.0;
                    self.advance();
                    let idx = self.parse_expr();
                    self.expect(Token::RBracket);
                    let span = (span_start, self.prev_end());
                    expr = Expr {
                        kind: ExprKind::Index {
                            obj: Box::new(expr),
                            idx: Box::new(idx),
                        },
                        span,
                    };
                }
                Token::Question => {
                    let span_start = expr.span.0;
                    self.advance();
                    let span = (span_start, self.prev_end());
                    expr = Expr {
                        kind: ExprKind::Propagate(Box::new(expr)),
                        span,
                    };
                }
                Token::Catch => {
                    let span_start = expr.span.0;
                    self.advance();
                    let err_name = self.expect_ident();
                    self.expect(Token::LBrace);
                    let body = self.parse_block();
                    self.expect(Token::RBrace);
                    let span = (span_start, self.prev_end());
                    expr = Expr {
                        kind: ExprKind::Catch {
                            expr: Box::new(expr),
                            err_name,
                            body,
                        },
                        span,
                    };
                }
                _ => break,
            }
        }

        expr
    }

    fn parse_primary(&mut self) -> Expr {
        match self.peek().clone() {
            Token::Switch => {
                let span_start = self.peek_span().0;
                self.advance();
                let expr = self.parse_expr();
                self.expect(Token::LBrace);
                let mut arms = Vec::new();
                while self.peek() != &Token::RBrace {
                    let pattern = match self.peek().clone() {
                        Token::Else => {
                            self.advance();
                            SwitchPattern::Else
                        }
                        Token::Ident(s) => {
                            self.advance();
                            SwitchPattern::Ident(s)
                        }
                        Token::Int(n) => {
                            self.advance();
                            SwitchPattern::Int(n)
                        }
                        Token::True => {
                            self.advance();
                            SwitchPattern::Bool(true)
                        }
                        Token::False => {
                            self.advance();
                            SwitchPattern::Bool(false)
                        }
                        tok => {
                            self.error(format!(
                                "Unexpected switch pattern: {}",
                                token_display(&tok)
                            ));
                            self.advance();
                            SwitchPattern::Else
                        }
                    };
                    self.expect(Token::FatArrow);
                    let body = if self.peek() == &Token::LBrace {
                        self.advance();
                        let stmts = self.parse_block();
                        self.expect(Token::RBrace);
                        if self.peek() == &Token::Comma {
                            self.advance();
                        }
                        SwitchBody::Block(stmts)
                    } else {
                        let e = self.parse_expr();
                        if self.peek() == &Token::Comma {
                            self.advance();
                        }
                        SwitchBody::Expr(e)
                    };
                    arms.push(SwitchArm { pattern, body });
                }
                self.expect(Token::RBrace);
                let span = (span_start, self.prev_end());
                Expr {
                    kind: ExprKind::Switch {
                        expr: Box::new(expr),
                        arms,
                    },
                    span,
                }
            }
            Token::If => {
                let span_start = self.peek_span().0;
                self.advance();
                let cond = self.parse_expr();
                self.expect(Token::Then);
                let then = self.parse_expr();
                self.expect(Token::Else);
                let else_ = self.parse_expr();
                let span = (span_start, self.prev_end());
                Expr {
                    kind: ExprKind::If {
                        cond: Box::new(cond),
                        then: Box::new(then),
                        else_: Box::new(else_),
                    },
                    span,
                }
            }
            Token::Import => {
                let span_start = self.peek_span().0;
                self.advance();
                self.expect(Token::LParen);
                let module = match self.advance() {
                    Token::Str(s) => s,
                    tok => {
                        self.error(format!(
                            "Expected string in import(), got {}",
                            token_display(&tok)
                        ));
                        String::new()
                    }
                };
                self.expect(Token::RParen);
                let span = (span_start, self.prev_end());
                Expr {
                    kind: ExprKind::Import(module),
                    span,
                }
            }
            Token::Ident(name) => {
                let span = self.peek_span();
                self.advance();
                Expr {
                    kind: ExprKind::Var(name),
                    span,
                }
            }
            Token::Str(s) => {
                let span = self.peek_span();
                self.advance();
                Expr {
                    kind: ExprKind::Str(s),
                    span,
                }
            }
            Token::Int(n) => {
                let span = self.peek_span();
                self.advance();
                Expr {
                    kind: ExprKind::Int(n),
                    span,
                }
            }
            Token::Float(f) => {
                let span = self.peek_span();
                self.advance();
                Expr {
                    kind: ExprKind::Float(f),
                    span,
                }
            }
            Token::True => {
                let span = self.peek_span();
                self.advance();
                Expr {
                    kind: ExprKind::Bool(true),
                    span,
                }
            }
            Token::False => {
                let span = self.peek_span();
                self.advance();
                Expr {
                    kind: ExprKind::Bool(false),
                    span,
                }
            }
            Token::LBracket => {
                let span_start = self.peek_span().0;
                self.advance();
                let mut elems = Vec::new();
                while self.peek() != &Token::RBracket {
                    elems.push(self.parse_expr());
                    if self.peek() == &Token::Comma {
                        self.advance();
                    }
                }
                self.expect(Token::RBracket);
                let span = (span_start, self.prev_end());
                Expr {
                    kind: ExprKind::ArrayLiteral(elems),
                    span,
                }
            }
            Token::LParen => {
                self.advance();
                let e = self.parse_expr();
                self.expect(Token::RParen);
                e
            }
            tok => {
                let span = self.peek_span();
                self.error(format!("Unexpected token: {}", token_display(&tok)));
                self.advance();
                Expr {
                    kind: ExprKind::Int(0),
                    span,
                }
            }
        }
    }
}

pub fn parse(tokens: Vec<(Token, Span)>) -> (Program, Vec<ParseError>) {
    let mut p = Parser::new(tokens);
    let program = p.parse_program();
    (program, p.errors)
}
