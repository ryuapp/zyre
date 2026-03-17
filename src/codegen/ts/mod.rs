use crate::codegen::Backend;
use crate::parser::*;
use std::collections::HashMap;

pub struct TsBackend {
    /// import("std") alias → "std"
    import_aliases: HashMap<String, String>,
    /// const fs = std.fs  →  alias → (module, ns)
    aliases: HashMap<String, (String, String)>,
    /// which std namespaces are used (e.g. "debug", "fs")
    used_std_ns: std::collections::HashSet<String>,
}

impl TsBackend {
    pub fn new() -> Self {
        Self {
            import_aliases: HashMap::new(),
            aliases: HashMap::new(),
            used_std_ns: std::collections::HashSet::new(),
        }
    }

    fn is_std(&self, name: &str) -> bool {
        self.import_aliases
            .get(name)
            .map(|m| m == "std")
            .unwrap_or(false)
    }

    // ---- program ----

    fn gen_program(&mut self, program: &Program) -> String {
        // Pass 1: collect import / alias consts
        for item in program {
            if let TopLevel::ConstDecl { name, value, .. } = item {
                match &value.kind {
                    ExprKind::Import(module) => {
                        self.import_aliases.insert(name.clone(), module.clone());
                    }
                    ExprKind::MemberAccess { obj, prop } => {
                        if let ExprKind::Var(base) = &obj.kind {
                            if self.is_std(base) {
                                self.aliases
                                    .insert(name.clone(), ("std".to_string(), prop.clone()));
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        // Pass 2: scan for std namespace usage
        self.scan_std_ns(program);

        let mut out = String::new();

        for (alias, module) in &self.import_aliases {
            if module.ends_with(".zy") {
                out.push_str(&format!(
                    "import * as {} from \"{}\";\n",
                    alias,
                    module.replace(".zy", ".ts")
                ));
            }
        }

        // Emit runtime imports for used std namespaces
        for ns in &["debug", "fs"] {
            if self.used_std_ns.contains(*ns) {
                out.push_str(&format!(
                    "import * as __zyre_std_{ns} from \"./zyre_std_{ns}.ts\";\n"
                ));
            }
        }
        if !self.import_aliases.is_empty() || !self.used_std_ns.is_empty() {
            out.push('\n');
        }

        // Declarations (struct / enum / fn)
        for item in program {
            match item {
                TopLevel::StructDecl {
                    name,
                    fields,
                    exported,
                } => {
                    let prefix = if *exported { "export " } else { "" };
                    out.push_str(&format!("{}interface {} {{\n", prefix, name));
                    for f in fields {
                        out.push_str(&format!("  {}: {};\n", f.name, self.gen_type(&f.ty)));
                    }
                    out.push_str("}\n\n");
                }
                TopLevel::EnumDecl {
                    name,
                    variants,
                    exported,
                } => {
                    let prefix = if *exported { "export " } else { "" };
                    out.push_str(&format!("{}const enum {} {{\n", prefix, name));
                    for v in variants {
                        out.push_str(&format!("  {},\n", v));
                    }
                    out.push_str("}\n\n");
                }
                TopLevel::FnDecl(f) => {
                    out.push_str(&self.gen_fn(f));
                    out.push('\n');
                }
                _ => {}
            }
        }

        // Top-level body: ConstDecl (non-import/alias) + Stmt
        for item in program {
            match item {
                TopLevel::ConstDecl {
                    name,
                    ty,
                    value,
                    exported,
                    ..
                } => {
                    if matches!(&value.kind, ExprKind::Import(_)) {
                        continue;
                    }
                    if let ExprKind::MemberAccess { obj, .. } = &value.kind {
                        if let ExprKind::Var(base) = &obj.kind {
                            if self.is_std(base) || self.import_aliases.contains_key(base) {
                                continue;
                            }
                        }
                    }
                    if let ExprKind::Var(src) = &value.kind {
                        if self.import_aliases.contains_key(src) || self.aliases.contains_key(src) {
                            continue;
                        }
                    }
                    let prefix = if *exported { "export " } else { "" };
                    let type_ann = ty
                        .as_ref()
                        .map(|t| format!(": {}", self.gen_type(t)))
                        .unwrap_or_default();
                    out.push_str(&format!(
                        "{}const {}{} = {};\n",
                        prefix,
                        name,
                        type_ann,
                        self.gen_expr(value)
                    ));
                }
                TopLevel::Stmt(s) => {
                    out.push_str(&self.gen_stmt(s, 0));
                }
                _ => {}
            }
        }

        if !out.ends_with('\n') {
            out.push('\n');
        }
        out
    }

    fn scan_std_ns(&mut self, program: &Program) {
        for item in program {
            match item {
                TopLevel::FnDecl(f) => self.scan_stmts(&f.body),
                TopLevel::Stmt(s) => self.scan_stmt(s),
                TopLevel::ConstDecl { value, .. } => self.scan_expr(value),
                _ => {}
            }
        }
    }

    fn scan_stmts(&mut self, stmts: &[Stmt]) {
        for s in stmts {
            self.scan_stmt(s);
        }
    }

    fn scan_stmt(&mut self, stmt: &Stmt) {
        match &stmt.kind {
            StmtKind::ConstDecl { value, .. } | StmtKind::LetDecl { value, .. } => {
                self.scan_expr(value)
            }
            StmtKind::Return(Some(e)) => self.scan_expr(e),
            StmtKind::ExprStmt(e) => self.scan_expr(e),
            StmtKind::If {
                cond,
                body,
                else_body,
            } => {
                self.scan_expr(cond);
                self.scan_stmts(body);
                if let Some(s) = else_body {
                    self.scan_stmts(s);
                }
            }
            StmtKind::While { cond, body } => {
                self.scan_expr(cond);
                self.scan_stmts(body);
            }
            _ => {}
        }
    }

    fn scan_expr(&mut self, expr: &Expr) {
        match &expr.kind {
            ExprKind::Call { callee, args } => {
                if let ExprKind::MemberAccess { obj, prop: _ } = &callee.kind {
                    // std.ns.fn(...)
                    if let ExprKind::MemberAccess {
                        obj: inner,
                        prop: ns,
                    } = &obj.kind
                    {
                        if let ExprKind::Var(base) = &inner.kind {
                            if self.is_std(base) {
                                self.used_std_ns.insert(ns.clone());
                            }
                        }
                    }
                    // alias.fn(...) where alias resolves to std.ns
                    if let ExprKind::Var(alias) = &obj.kind {
                        if let Some((m, ns)) = self.aliases.get(alias) {
                            if m == "std" {
                                self.used_std_ns.insert(ns.clone());
                            }
                        }
                    }
                }
                self.scan_expr(callee);
                for a in args {
                    self.scan_expr(a);
                }
            }
            ExprKind::MemberAccess { obj, .. } => self.scan_expr(obj),
            ExprKind::BinOp { lhs, rhs, .. } => {
                self.scan_expr(lhs);
                self.scan_expr(rhs);
            }
            ExprKind::UnOp { expr, .. } => self.scan_expr(expr),
            ExprKind::Propagate(e) => self.scan_expr(e),
            ExprKind::Catch { expr, body, .. } => {
                self.scan_expr(expr);
                self.scan_stmts(body);
            }
            ExprKind::Switch { expr, arms } => {
                self.scan_expr(expr);
                for arm in arms {
                    match &arm.body {
                        SwitchBody::Expr(e) => self.scan_expr(e),
                        SwitchBody::Block(stmts) => self.scan_stmts(stmts),
                    }
                }
            }
            ExprKind::ArrayLiteral(elems) => {
                for e in elems {
                    self.scan_expr(e);
                }
            }
            ExprKind::Index { obj, idx } => {
                self.scan_expr(obj);
                self.scan_expr(idx);
            }
            ExprKind::If { cond, then, else_ } => {
                self.scan_expr(cond);
                self.scan_expr(then);
                self.scan_expr(else_);
            }
            _ => {}
        }
    }

    // ---- types ----

    fn gen_type(&self, ty: &TypeExpr) -> String {
        match ty {
            TypeExpr::Name(n) => match n.as_str() {
                "i32" | "f64" => "number".to_string(),
                "string" => "string".to_string(),
                "bool" => "boolean".to_string(),
                "void" => "void".to_string(),
                other => other.to_string(),
            },
            TypeExpr::Array(inner, _) => format!("{}[]", self.gen_type(inner)),
            TypeExpr::Optional(inner) => format!("{} | null", self.gen_type(inner)),
            // Error unions: strip the error, just use the inner type (throw-based)
            TypeExpr::ErrorUnion(_, inner) => self.gen_type(inner),
        }
    }

    // ---- functions ----

    fn gen_fn(&self, f: &FnDecl) -> String {
        let prefix = if f.exported { "export " } else { "" };
        let params: Vec<String> = f
            .params
            .iter()
            .map(|(name, ty)| format!("{}: {}", name, self.gen_type(ty)))
            .collect();
        let ret = self.gen_type(&f.ret);
        let mut out = format!(
            "{}function {}({}): {} {{\n",
            prefix,
            f.name,
            params.join(", "),
            ret
        );
        for stmt in &f.body {
            out.push_str(&self.gen_stmt(stmt, 1));
        }
        out.push('}');
        out
    }

    // ---- statements ----

    fn gen_stmt(&self, stmt: &Stmt, level: usize) -> String {
        let ind = "  ".repeat(level);
        match &stmt.kind {
            StmtKind::ConstDecl { name, ty, value } => {
                let type_ann = ty
                    .as_ref()
                    .map(|t| format!(": {}", self.gen_type(t)))
                    .unwrap_or_default();
                format!(
                    "{}const {}{} = {};\n",
                    ind,
                    name,
                    type_ann,
                    self.gen_expr(value)
                )
            }
            StmtKind::LetDecl { name, ty, value } => {
                let type_ann = ty
                    .as_ref()
                    .map(|t| format!(": {}", self.gen_type(t)))
                    .unwrap_or_default();
                format!(
                    "{}let {}{} = {};\n",
                    ind,
                    name,
                    type_ann,
                    self.gen_expr(value)
                )
            }
            StmtKind::Return(None) => format!("{}return;\n", ind),
            StmtKind::Return(Some(e)) => format!("{}return {};\n", ind, self.gen_expr(e)),
            StmtKind::ExprStmt(e) => format!("{}{};\n", ind, self.gen_expr(e)),
            StmtKind::If {
                cond,
                body,
                else_body,
            } => {
                let mut out = format!("{}if ({}) {{\n", ind, self.gen_expr(cond));
                for s in body {
                    out.push_str(&self.gen_stmt(s, level + 1));
                }
                if let Some(else_stmts) = else_body {
                    out.push_str(&format!("{}}} else {{\n", ind));
                    for s in else_stmts {
                        out.push_str(&self.gen_stmt(s, level + 1));
                    }
                }
                out.push_str(&format!("{}}}\n", ind));
                out
            }
            StmtKind::While { cond, body } => {
                let mut out = format!("{}while ({}) {{\n", ind, self.gen_expr(cond));
                for s in body {
                    out.push_str(&self.gen_stmt(s, level + 1));
                }
                out.push_str(&format!("{}}}\n", ind));
                out
            }
            StmtKind::Break => format!("{}break;\n", ind),
            StmtKind::Continue => format!("{}continue;\n", ind),
        }
    }

    // ---- expressions ----

    fn gen_expr(&self, expr: &Expr) -> String {
        match &expr.kind {
            ExprKind::Var(name) => name.clone(),
            ExprKind::Str(s) => format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\"")),
            ExprKind::Int(n) => n.to_string(),
            ExprKind::Float(f) => {
                if f.fract() == 0.0 {
                    format!("{:.1}", f)
                } else {
                    f.to_string()
                }
            }
            ExprKind::Bool(b) => b.to_string(),
            ExprKind::Import(_) => String::new(),

            ExprKind::MemberAccess { obj, prop } => {
                format!("{}.{}", self.gen_expr(obj), prop)
            }

            ExprKind::Call { callee, args } => self.gen_call(callee, args),

            ExprKind::BinOp { op, lhs, rhs } => {
                let op_s = match op {
                    BinOp::Add => "+",
                    BinOp::Sub => "-",
                    BinOp::Mul => "*",
                    BinOp::Div => "/",
                    BinOp::Mod => "%",
                    BinOp::Eq => "===",
                    BinOp::NotEq => "!==",
                    BinOp::Lt => "<",
                    BinOp::Gt => ">",
                    BinOp::LtEq => "<=",
                    BinOp::GtEq => ">=",
                    BinOp::And => "&&",
                    BinOp::Or => "||",
                };
                format!("({} {} {})", self.gen_expr(lhs), op_s, self.gen_expr(rhs))
            }

            ExprKind::UnOp { op, expr } => match op {
                UnOp::Neg => format!("(-{})", self.gen_expr(expr)),
                UnOp::Not => format!("(!{})", self.gen_expr(expr)),
            },

            // error propagation: in throw-based TS errors propagate naturally
            ExprKind::Propagate(e) => self.gen_expr(e),

            // catch expr → IIFE with try/catch
            ExprKind::Catch {
                expr,
                err_name,
                body,
            } => {
                let err_bind = if err_name.starts_with('_') {
                    "_e".to_string()
                } else {
                    err_name.clone()
                };
                let inner = self.gen_expr(expr);
                let mut out = format!(
                    "(() => {{ try {{ return {}; }} catch ({}) {{",
                    inner, err_bind
                );
                for s in body {
                    out.push(' ');
                    out.push_str(self.gen_stmt(s, 0).trim_end());
                    out.push(';');
                }
                out.push_str(" } })()");
                out
            }

            ExprKind::Switch { expr, arms } => {
                let subj = self.gen_expr(expr);
                let mut out = format!("(() => {{ switch ({}) {{", subj);
                for arm in arms {
                    let pat = match &arm.pattern {
                        SwitchPattern::Ident(s) => s.clone(),
                        SwitchPattern::Int(n) => n.to_string(),
                        SwitchPattern::Bool(b) => b.to_string(),
                        SwitchPattern::Else => {
                            match &arm.body {
                                SwitchBody::Expr(e) => {
                                    out.push_str(&format!(
                                        " default: return {};",
                                        self.gen_expr(e)
                                    ));
                                }
                                SwitchBody::Block(stmts) => {
                                    out.push_str(" default: {");
                                    for s in stmts {
                                        out.push(' ');
                                        out.push_str(self.gen_stmt(s, 0).trim_end());
                                        out.push(';');
                                    }
                                    out.push('}');
                                }
                            }
                            continue;
                        }
                    };
                    match &arm.body {
                        SwitchBody::Expr(e) => {
                            out.push_str(&format!(" case {}: return {};", pat, self.gen_expr(e)));
                        }
                        SwitchBody::Block(stmts) => {
                            out.push_str(&format!(" case {}: {{", pat));
                            for s in stmts {
                                out.push(' ');
                                out.push_str(self.gen_stmt(s, 0).trim_end());
                                out.push(';');
                            }
                            out.push_str(" break; }");
                        }
                    }
                }
                out.push_str(" } })()");
                out
            }

            ExprKind::ArrayLiteral(elems) => {
                let s: Vec<String> = elems.iter().map(|e| self.gen_expr(e)).collect();
                format!("[{}]", s.join(", "))
            }

            ExprKind::Index { obj, idx } => {
                format!("{}[{}]", self.gen_expr(obj), self.gen_expr(idx))
            }

            ExprKind::If { cond, then, else_ } => {
                format!(
                    "({} ? {} : {})",
                    self.gen_expr(cond),
                    self.gen_expr(then),
                    self.gen_expr(else_)
                )
            }
        }
    }

    // ---- call dispatch ----

    fn gen_call(&self, callee: &Expr, args: &[Expr]) -> String {
        // std.debug.print / std.fs.readTextFile etc.
        if let ExprKind::MemberAccess { obj, prop } = &callee.kind {
            // std.ns.fn(args)
            if let ExprKind::MemberAccess {
                obj: inner,
                prop: ns,
            } = &obj.kind
            {
                if let ExprKind::Var(base) = &inner.kind {
                    if self.is_std(base) {
                        return self.gen_std_ns_call(ns, prop, args);
                    }
                }
            }
            // alias.fn(args)  where alias resolves to std.ns
            if let ExprKind::Var(alias) = &obj.kind {
                if let Some((m, ns)) = self.aliases.get(alias) {
                    if m == "std" {
                        return self.gen_std_ns_call(ns, prop, args);
                    }
                }
                // direct std module alias: const s = import("std"); s.debug.print(...)
                if self.is_std(alias) {
                    return self.gen_std_ns_call(prop, prop, args);
                }
            }
        }

        let callee_s = self.gen_expr(callee);
        let args_s: Vec<String> = args.iter().map(|a| self.gen_expr(a)).collect();
        format!("{}({})", callee_s, args_s.join(", "))
    }

    fn gen_std_ns_call(&self, ns: &str, fn_name: &str, args: &[Expr]) -> String {
        let args_s: Vec<String> = args.iter().map(|a| self.gen_expr(a)).collect();
        match ns {
            "debug" | "fs" => format!("__zyre_std_{}.{}({})", ns, fn_name, args_s.join(", ")),
            _ => format!(
                "/* std.{}.{} */ {}({})",
                ns,
                fn_name,
                fn_name,
                args_s.join(", ")
            ),
        }
    }
}

impl Backend for TsBackend {
    fn generate(&mut self, program: &Program) -> String {
        self.gen_program(program)
    }
}
