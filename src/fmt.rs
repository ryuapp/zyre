use crate::parser::{
    BinOp, Expr, ExprKind, FnDecl, StmtKind, SwitchArm, SwitchBody, SwitchPattern, TopLevel,
    TypeExpr, UnOp,
};

struct Formatter {
    buf: String,
    indent: usize,
}

impl Formatter {
    fn new() -> Self {
        Self {
            buf: String::new(),
            indent: 0,
        }
    }

    fn newline(&mut self) {
        self.buf.push('\n');
    }

    fn indent_str(&self) -> String {
        "    ".repeat(self.indent)
    }

    fn line(&mut self, s: &str) {
        for _ in 0..self.indent {
            self.buf.push_str("    ");
        }
        self.buf.push_str(s);
        self.buf.push('\n');
    }

    /// Format a statement and return it as a String without writing to `self.buf`.
    fn fmt_stmt_str(&mut self, stmt: &crate::parser::Stmt) -> String {
        let old_len = self.buf.len();
        self.fmt_stmt(stmt);
        let added = self.buf[old_len..].to_string();
        self.buf.truncate(old_len);
        added
    }

    fn format_program(&mut self, program: &crate::parser::Program, blanks: &[bool]) {
        let mut prev_was_block = false;
        for (i, item) in program.iter().enumerate() {
            let is_block = matches!(
                item,
                TopLevel::FnDecl(_) | TopLevel::StructDecl { .. } | TopLevel::EnumDecl { .. }
            );
            let blank_before = blanks.get(i).copied().unwrap_or(false);
            if i > 0 && (is_block || prev_was_block || blank_before) {
                self.newline();
            }
            self.fmt_toplevel(item);
            prev_was_block = is_block;
        }
    }

    fn fmt_toplevel(&mut self, item: &TopLevel) {
        match item {
            TopLevel::ConstDecl {
                name,
                value,
                exported,
                ..
            } => {
                let prefix = if *exported { "export " } else { "" };
                let val = self.fmt_expr(value);
                self.line(&format!("{}const {} = {}", prefix, name, val));
            }
            TopLevel::FnDecl(f) => self.fmt_fn(f),
            TopLevel::StructDecl {
                name,
                fields,
                exported,
            } => {
                let prefix = if *exported { "export " } else { "" };
                self.line(&format!("{}const {} = struct {{", prefix, name));
                self.indent += 1;
                for field in fields {
                    let ty = fmt_type(&field.ty);
                    self.line(&format!("{}: {},", field.name, ty));
                }
                self.indent -= 1;
                self.line("};");
            }
            TopLevel::EnumDecl {
                name,
                variants,
                exported,
            } => {
                let prefix = if *exported { "export " } else { "" };
                self.line(&format!("{}const {} = enum {{", prefix, name));
                self.indent += 1;
                for v in variants {
                    self.line(&format!("{},", v));
                }
                self.indent -= 1;
                self.line("};");
            }
            TopLevel::Stmt(stmt) => self.fmt_stmt(stmt),
        }
    }

    fn fmt_fn(&mut self, f: &FnDecl) {
        let prefix = if f.exported { "export " } else { "" };
        let params: Vec<String> = f
            .params
            .iter()
            .map(|(name, ty)| format!("{}: {}", name, fmt_type(ty)))
            .collect();
        let ret = fmt_type(&f.ret);
        self.line(&format!(
            "{}fn {}({}): {} {{",
            prefix,
            f.name,
            params.join(", "),
            ret
        ));
        self.indent += 1;
        for stmt in &f.body {
            self.fmt_stmt(stmt);
        }
        self.indent -= 1;
        self.line("}");
    }

    fn fmt_stmt(&mut self, stmt: &crate::parser::Stmt) {
        match &stmt.kind {
            StmtKind::ConstDecl { name, ty, value } => {
                let val = self.fmt_expr(value);
                if let Some(t) = ty {
                    self.line(&format!("const {}: {} = {}", name, fmt_type(t), val));
                } else {
                    self.line(&format!("const {} = {}", name, val));
                }
            }
            StmtKind::LetDecl { name, ty, value } => {
                let val = self.fmt_expr(value);
                if let Some(t) = ty {
                    self.line(&format!("let {}: {} = {}", name, fmt_type(t), val));
                } else {
                    self.line(&format!("let {} = {}", name, val));
                }
            }
            StmtKind::Return(None) => self.line("return"),
            StmtKind::Return(Some(e)) => {
                let val = self.fmt_expr(e);
                self.line(&format!("return {}", val));
            }
            StmtKind::If {
                cond,
                body,
                else_body,
            } => {
                let c = self.fmt_expr(cond);
                self.line(&format!("if {} {{", c));
                self.indent += 1;
                for s in body {
                    self.fmt_stmt(s);
                }
                self.indent -= 1;
                if let Some(else_stmts) = else_body {
                    self.line("} else {");
                    self.indent += 1;
                    for s in else_stmts {
                        self.fmt_stmt(s);
                    }
                    self.indent -= 1;
                    self.line("}");
                } else {
                    self.line("}");
                }
            }
            StmtKind::While { cond, body } => {
                let c = self.fmt_expr(cond);
                self.line(&format!("while {} {{", c));
                self.indent += 1;
                for s in body {
                    self.fmt_stmt(s);
                }
                self.indent -= 1;
                self.line("}");
            }
            StmtKind::Break => self.line("break"),
            StmtKind::Continue => self.line("continue"),
            StmtKind::ExprStmt(e) => {
                let val = self.fmt_expr(e);
                self.line(&val);
            }
        }
    }

    fn fmt_expr(&mut self, expr: &Expr) -> String {
        match &expr.kind {
            ExprKind::Import(path) => format!("import(\"{}\")", path),
            ExprKind::Var(s) => s.clone(),
            ExprKind::Str(s) => format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\"")),
            ExprKind::Int(n) => n.to_string(),
            ExprKind::Float(f) => {
                if f.fract() == 0.0 {
                    format!("{:.1}", f)
                } else {
                    format!("{}", f)
                }
            }
            ExprKind::Bool(b) => b.to_string(),
            ExprKind::MemberAccess { obj, prop } => {
                format!("{}.{}", self.fmt_expr(obj), prop)
            }
            ExprKind::Call { callee, args } => {
                let c = self.fmt_expr(callee);
                let a: Vec<String> = args.iter().map(|a| self.fmt_expr(a)).collect();
                format!("{}({})", c, a.join(", "))
            }
            ExprKind::Index { obj, idx } => {
                format!("{}[{}]", self.fmt_expr(obj), self.fmt_expr(idx))
            }
            ExprKind::Propagate(e) => format!("{}?", self.fmt_expr(e)),
            ExprKind::Catch {
                expr,
                err_name,
                body,
            } => {
                let e = self.fmt_expr(expr);
                let header = format!("{} catch {} {{", e, err_name);
                let mut s = header;
                s.push('\n');
                self.indent += 1;
                for stmt in body {
                    s.push_str(&self.indent_str());
                    s.push_str(self.fmt_stmt_str(stmt).trim_start());
                }
                self.indent -= 1;
                s.push_str(&self.indent_str());
                s.push('}');
                s
            }
            ExprKind::Switch { expr, arms } => self.fmt_switch(expr, arms),
            ExprKind::ArrayLiteral(elems) => {
                let e: Vec<String> = elems.iter().map(|e| self.fmt_expr(e)).collect();
                format!("[{}]", e.join(", "))
            }
            ExprKind::BinOp { op, lhs, rhs } => {
                let l = self.fmt_binop_side(op, lhs, false);
                let r = self.fmt_binop_side(op, rhs, true);
                format!("{} {} {}", l, binop_str(op), r)
            }
            ExprKind::UnOp { op, expr } => match op {
                UnOp::Neg => format!("-{}", self.fmt_unop_operand(expr)),
                UnOp::Not => format!("!{}", self.fmt_unop_operand(expr)),
            },
            ExprKind::If { cond, then, else_ } => {
                format!(
                    "if {} then {} else {}",
                    self.fmt_expr(cond),
                    self.fmt_expr(then),
                    self.fmt_expr(else_)
                )
            }
        }
    }

    fn fmt_binop_side(&mut self, parent: &BinOp, child: &Expr, is_rhs: bool) -> String {
        let s = self.fmt_expr(child);
        if let ExprKind::BinOp { op, .. } = &child.kind {
            let cp = precedence(op);
            let pp = precedence(parent);
            if cp < pp || (is_rhs && cp == pp) {
                return format!("({})", s);
            }
        }
        s
    }

    fn fmt_unop_operand(&mut self, expr: &Expr) -> String {
        let s = self.fmt_expr(expr);
        if matches!(expr.kind, ExprKind::BinOp { .. }) {
            format!("({})", s)
        } else {
            s
        }
    }

    fn fmt_switch(&mut self, expr: &Expr, arms: &[SwitchArm]) -> String {
        let e = self.fmt_expr(expr);
        let mut s = format!("switch {} {{\n", e);
        self.indent += 1;
        for arm in arms {
            let pat = match &arm.pattern {
                SwitchPattern::Ident(n) => n.clone(),
                SwitchPattern::Int(n) => n.to_string(),
                SwitchPattern::Bool(b) => b.to_string(),
                SwitchPattern::Else => "else".to_string(),
            };
            match &arm.body {
                SwitchBody::Expr(e) => {
                    let val = self.fmt_expr(e);
                    s.push_str(&self.indent_str());
                    s.push_str(&format!("{} => {},\n", pat, val));
                }
                SwitchBody::Block(stmts) => {
                    s.push_str(&self.indent_str());
                    s.push_str(&format!("{} => {{\n", pat));
                    self.indent += 1;
                    for stmt in stmts {
                        s.push_str(&self.indent_str());
                        s.push_str(self.fmt_stmt_str(stmt).trim_start());
                    }
                    self.indent -= 1;
                    s.push_str(&self.indent_str());
                    s.push_str("},\n");
                }
            }
        }
        self.indent -= 1;
        s.push_str(&self.indent_str());
        s.push('}');
        s
    }
}

fn fmt_type(ty: &TypeExpr) -> String {
    match ty {
        TypeExpr::Name(s) => s.clone(),
        TypeExpr::Array(inner, n) => format!("{}[{}]", fmt_type(inner), n),
        TypeExpr::Optional(inner) => format!("?{}", fmt_type(inner)),
        TypeExpr::ErrorUnion(None, t) => format!("!{}", fmt_type(t)),
        TypeExpr::ErrorUnion(Some(e), t) => format!("{}!{}", fmt_type(e), fmt_type(t)),
    }
}

fn binop_str(op: &BinOp) -> &'static str {
    match op {
        BinOp::Add => "+",
        BinOp::Sub => "-",
        BinOp::Mul => "*",
        BinOp::Div => "/",
        BinOp::Mod => "%",
        BinOp::Eq => "==",
        BinOp::NotEq => "!=",
        BinOp::Lt => "<",
        BinOp::Gt => ">",
        BinOp::LtEq => "<=",
        BinOp::GtEq => ">=",
        BinOp::And => "and",
        BinOp::Or => "or",
    }
}

fn precedence(op: &BinOp) -> u8 {
    match op {
        BinOp::Or => 1,
        BinOp::And => 2,
        BinOp::Eq | BinOp::NotEq | BinOp::Lt | BinOp::Gt | BinOp::LtEq | BinOp::GtEq => 3,
        BinOp::Add | BinOp::Sub => 4,
        BinOp::Mul | BinOp::Div | BinOp::Mod => 5,
    }
}

pub fn format_program(program: &crate::parser::Program, blanks: &[bool]) -> String {
    let mut f = Formatter::new();
    f.format_program(program, blanks);
    if !f.buf.ends_with('\n') {
        f.buf.push('\n');
    }
    f.buf
}
