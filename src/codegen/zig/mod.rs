use crate::codegen::Backend;
use crate::parser::*;
use std::collections::{HashMap, HashSet};

const MAIN_MANGLED: &str = "__zyre_fn_main";

mod stdlib;
mod tracker;

pub struct ZigBackend {
    pub(super) imports: Vec<String>,
    // Aliases like const fs = std.fs: alias -> (resolved_module, ns)
    pub(super) aliases: HashMap<String, (String, String)>,
    // Aliases like const s = import("std"): alias -> module name ("s" -> "std")
    pub(super) import_aliases: HashMap<String, String>,
    pub(super) catch_err_vars: HashSet<String>,
    // Names of functions that require an allocator (transitively)
    pub(super) allocating_fns: HashSet<String>,
}

impl ZigBackend {
    pub fn new() -> Self {
        ZigBackend {
            imports: Vec::new(),
            aliases: HashMap::new(),
            import_aliases: HashMap::new(),
            catch_err_vars: HashSet::new(),
            allocating_fns: HashSet::new(),
        }
    }

    /// Resolves an alias name to the actual module name (e.g. "s" -> "std")
    pub(super) fn resolve_module<'a>(&'a self, name: &'a str) -> &'a str {
        self.import_aliases
            .get(name)
            .map(|s| s.as_str())
            .unwrap_or(name)
    }

    fn zy_to_zig_path(path: &str) -> String {
        if path.ends_with(".zy") {
            path[..path.len() - 3].to_string() + ".zig"
        } else {
            path.to_string()
        }
    }

    pub(super) fn is_std_module(&self, name: &str) -> bool {
        self.resolve_module(name) == "std"
    }

    pub(super) fn gen_args(&mut self, args: &[Expr]) -> Vec<String> {
        args.iter().map(|a| self.gen_expr(a)).collect()
    }
}

impl Backend for ZigBackend {
    fn generate(&mut self, program: &Program) -> String {
        self.gen_program(program)
    }
}

impl ZigBackend {
    /// Recursively collects export const names and their const dependencies
    fn collect_hoisted(program: &Program) -> HashSet<String> {
        // Build a map of name -> value
        let const_map: HashMap<String, &Expr> = program
            .iter()
            .filter_map(|item| {
                if let TopLevel::ConstDecl { name, value, .. } = item {
                    Some((name.clone(), value))
                } else {
                    None
                }
            })
            .collect();

        let mut hoisted = HashSet::new();

        // Recursively collect dependencies starting from export const
        fn collect_deps(
            name: &str,
            const_map: &HashMap<String, &Expr>,
            hoisted: &mut HashSet<String>,
        ) {
            if hoisted.contains(name) {
                return;
            }
            if let Some(expr) = const_map.get(name) {
                hoisted.insert(name.to_string());
                collect_expr_deps(expr, const_map, hoisted);
            }
        }

        fn collect_expr_deps(
            expr: &Expr,
            const_map: &HashMap<String, &Expr>,
            hoisted: &mut HashSet<String>,
        ) {
            match &expr.kind {
                ExprKind::Var(name) => collect_deps(name, const_map, hoisted),
                ExprKind::BinOp { lhs, rhs, .. } => {
                    collect_expr_deps(lhs, const_map, hoisted);
                    collect_expr_deps(rhs, const_map, hoisted);
                }
                ExprKind::UnOp { expr, .. } => collect_expr_deps(expr, const_map, hoisted),
                ExprKind::Call { callee, args } => {
                    collect_expr_deps(callee, const_map, hoisted);
                    for a in args {
                        collect_expr_deps(a, const_map, hoisted);
                    }
                }
                ExprKind::MemberAccess { obj, .. } => collect_expr_deps(obj, const_map, hoisted),
                _ => {}
            }
        }

        for item in program {
            if let TopLevel::ConstDecl {
                name,
                exported: true,
                ..
            } = item
            {
                collect_deps(name, &const_map, &mut hoisted);
            }
        }

        hoisted
    }

    fn gen_program(&mut self, program: &Program) -> String {
        // Pass 1: collect import names and aliases (must run before allocating_fns detection)
        let mut import_names: HashSet<String> = HashSet::new();
        // User-defined imports other than std (name, zig_path)
        let mut user_imports: Vec<(String, String)> = Vec::new();
        for item in program {
            if let TopLevel::ConstDecl { name, value, .. } = item {
                if let ExprKind::Import(module) = &value.kind {
                    self.imports.push(module.clone());
                    import_names.insert(name.clone());
                    self.import_aliases.insert(name.clone(), module.clone());
                    if !self.is_std_module(module) {
                        user_imports.push((name.clone(), Self::zy_to_zig_path(module)));
                    }
                }
                if let ExprKind::MemberAccess { obj, prop } = &value.kind {
                    if let ExprKind::Var(module) = &obj.kind {
                        let resolved = self.resolve_module(module).to_string();
                        self.aliases.insert(name.clone(), (resolved, prop.clone()));
                    }
                }
                if let ExprKind::Var(src) = &value.kind {
                    if let Some(module) = self.import_aliases.get(src).cloned() {
                        self.import_aliases.insert(name.clone(), module);
                        import_names.insert(name.clone());
                    } else if let Some(resolved) = self.aliases.get(src).cloned() {
                        self.aliases.insert(name.clone(), resolved);
                    }
                }
            }
        }

        // Detect allocating_fns after alias collection (to correctly detect readTextFile via aliases)
        self.allocating_fns = self.collect_allocating_fns(program);

        // Hoisting: collect export const and their dependencies
        let hoisted = Self::collect_hoisted(program);

        // Pass 2: collect body_stmts first (needed for header generation)
        let mut body_stmts: Vec<Stmt> = Vec::new();
        for item in program {
            match item {
                TopLevel::ConstDecl {
                    name, ty, value, ..
                } => {
                    let is_import = matches!(&value.kind, ExprKind::Import(_));
                    let is_module_alias = if let ExprKind::MemberAccess { obj, .. } = &value.kind {
                        matches!(&obj.kind, ExprKind::Var(m) if import_names.contains(m) || self.aliases.contains_key(m))
                    } else {
                        false
                    };
                    let is_var_alias = if let ExprKind::Var(src) = &value.kind {
                        self.aliases.contains_key(src) || import_names.contains(src)
                    } else {
                        false
                    };
                    if !is_import && !is_module_alias && !is_var_alias && !hoisted.contains(name) {
                        body_stmts.push(Stmt {
                            kind: StmtKind::ConstDecl {
                                name: name.clone(),
                                ty: ty.clone(),
                                value: value.clone(),
                            },
                            span: (0, 0),
                        });
                    }
                }
                TopLevel::Stmt(stmt) => body_stmts.push(stmt.clone()),
                _ => {}
            }
        }

        // implicit main uses std.heap, so std is required
        let needs_std = self.program_uses_std(program) || !body_stmts.is_empty();

        let mut out = String::new();
        if needs_std {
            out.push_str("const std = @import(\"std\");\n");
            out.push_str("const __zyre_runtime = @import(\"zyre_runtime.zig\");\n");
            out.push_str("const __zyre_std_debug = @import(\"zyre_std_debug.zig\");\n");
            out.push_str("const __zyre_std_fs = @import(\"zyre_std_fs.zig\");\n");
        }
        for (name, path) in &user_imports {
            out.push_str(&format!("const {} = @import(\"{}\");\n", name, path));
        }
        if needs_std || !user_imports.is_empty() {
            out.push('\n');
        }

        // Emit hoisted export consts at the top level (preserving declaration order)
        for item in program {
            if let TopLevel::ConstDecl {
                name,
                value,
                exported,
                ..
            } = item
            {
                if hoisted.contains(name) && !import_names.contains(name) {
                    let pub_prefix = if *exported { "pub " } else { "" };
                    out.push_str(&format!(
                        "{}const {} = {};\n",
                        pub_prefix,
                        name,
                        self.gen_expr(value)
                    ));
                }
            }
        }
        if hoisted.iter().any(|n| !import_names.contains(n)) {
            out.push('\n');
        }

        for item in program {
            match item {
                TopLevel::StructDecl {
                    name,
                    fields,
                    exported,
                } => {
                    let pub_prefix = if *exported { "pub " } else { "" };
                    out.push_str(&format!("{}const {} = struct {{\n", pub_prefix, name));
                    for f in fields {
                        out.push_str(&format!("    {}: {},\n", f.name, self.gen_type(&f.ty)));
                    }
                    out.push_str("};\n\n");
                }
                TopLevel::EnumDecl {
                    name,
                    variants,
                    exported,
                } => {
                    let pub_prefix = if *exported { "pub " } else { "" };
                    out.push_str(&format!("{}const {} = enum {{\n", pub_prefix, name));
                    for v in variants {
                        out.push_str(&format!("    {},\n", v));
                    }
                    out.push_str("};\n\n");
                }
                TopLevel::FnDecl(f) => {
                    out.push_str(&self.gen_fn(f));
                    out.push('\n');
                }
                // ConstDecl and Stmt were already collected in pass 2
                TopLevel::ConstDecl { .. } | TopLevel::Stmt(_) => {}
            }
        }

        if !body_stmts.is_empty() {
            let needs_alloc = self.uses_allocator(&body_stmts);
            out.push_str("pub fn main() !void {\n");
            out.push_str("    try __zyre_runtime.Output.init();\n");
            out.push_str("    defer __zyre_runtime.Output.restore();\n");
            out.push_str(&Self::gen_arena_setup(needs_alloc));
            for stmt in &body_stmts {
                out.push_str(&self.gen_stmt(stmt, 1));
            }
            out.push_str("}\n");
        }

        out
    }

    pub(super) fn gen_type(&self, ty: &TypeExpr) -> String {
        match ty {
            TypeExpr::Name(name) => match name.as_str() {
                "i32" => "i32".to_string(),
                "f64" => "f64".to_string(),
                "string" => "[]const u8".to_string(),
                "bool" => "bool".to_string(),
                "void" => "void".to_string(),
                other => other.to_string(),
            },
            TypeExpr::Array(inner, n) => format!("[{}]{}", n, self.gen_type(inner)),
            TypeExpr::Optional(inner) => format!("?{}", self.gen_type(inner)),
            TypeExpr::ErrorUnion(None, inner) => format!("!{}", self.gen_type(inner)),
            TypeExpr::ErrorUnion(Some(e), inner) => {
                format!("{}!{}", self.gen_type(e), self.gen_type(inner))
            }
        }
    }

    fn is_allocating_call(&self, callee: &Expr) -> bool {
        if let ExprKind::Var(name) = &callee.kind {
            self.allocating_fns.contains(name)
        } else {
            false
        }
    }

    fn gen_fn(&mut self, f: &FnDecl) -> String {
        let params: Vec<String> = f
            .params
            .iter()
            .map(|(name, ty)| format!("{}: {}", name, self.gen_type(ty)))
            .collect();

        let pub_prefix = if f.exported { "pub " } else { "" };
        let ret = self.gen_type(&f.ret);

        let needs_alloc = self.allocating_fns.contains(&f.name);

        let params_str = if needs_alloc {
            let mut all = vec!["__zyre_allocator: std.mem.Allocator".to_string()];
            all.extend(params);
            all.join(", ")
        } else {
            params.join(", ")
        };

        // "main" is reserved for the implicit entry point in Zig; mangle user-defined fn main
        let zig_name = if f.name == "main" {
            MAIN_MANGLED
        } else {
            &f.name
        };
        let mut out = format!("{}fn {}({}) {} {{\n", pub_prefix, zig_name, params_str, ret);

        for stmt in &f.body {
            out.push_str(&self.gen_stmt(stmt, 1));
        }

        out.push_str("}\n");
        out
    }

    fn gen_arena_setup(needs_alloc: bool) -> String {
        let mut s = String::new();
        s.push_str(
            "    var __zyre_arena = std.heap.ArenaAllocator.init(std.heap.page_allocator);\n",
        );
        s.push_str("    defer __zyre_arena.deinit();\n");
        s.push_str("    const __zyre_allocator = __zyre_arena.allocator();\n");
        if !needs_alloc {
            s.push_str("    _ = __zyre_allocator;\n");
        }
        s.push('\n');
        s
    }

    fn indent(level: usize) -> String {
        "    ".repeat(level)
    }

    fn gen_cond_block(
        &mut self,
        keyword: &str,
        cond: &Expr,
        body: &[Stmt],
        level: usize,
    ) -> String {
        let ind = Self::indent(level);
        let cond_s = self.gen_expr(cond);
        let mut out = format!("{}{} ({}) {{\n", ind, keyword, cond_s);
        for s in body {
            out.push_str(&self.gen_stmt(s, level + 1));
        }
        out.push_str(&format!("{}}}\n", ind));
        out
    }

    fn gen_type_annotation(&self, ty: &Option<TypeExpr>) -> String {
        ty.as_ref()
            .map(|t| format!(": {}", self.gen_type(t)))
            .unwrap_or_default()
    }

    pub(super) fn gen_stmt(&mut self, stmt: &Stmt, level: usize) -> String {
        let ind = Self::indent(level);
        match &stmt.kind {
            StmtKind::ConstDecl { name, ty, value } => {
                format!(
                    "{}const {}{} = {};\n",
                    ind,
                    name,
                    self.gen_type_annotation(ty),
                    self.gen_expr(value)
                )
            }
            StmtKind::LetDecl { name, ty, value } => {
                format!(
                    "{}var {}{} = {};\n",
                    ind,
                    name,
                    self.gen_type_annotation(ty),
                    self.gen_expr(value)
                )
            }
            StmtKind::Return(None) => format!("{}return;\n", ind),
            StmtKind::Return(Some(e)) => format!("{}return {};\n", ind, self.gen_expr(e)),
            StmtKind::Break => format!("{}break;\n", ind),
            StmtKind::Continue => format!("{}continue;\n", ind),
            StmtKind::If {
                cond,
                body,
                else_body,
            } => {
                let mut s = self.gen_cond_block("if", cond, body, level);
                if let Some(else_stmts) = else_body {
                    let ind = Self::indent(level);
                    s.pop(); // remove trailing '\n'
                    s.push_str(" else {\n");
                    for stmt in else_stmts {
                        s.push_str(&self.gen_stmt(stmt, level + 1));
                    }
                    s.push_str(&format!("{}}}\n", ind));
                }
                s
            }
            StmtKind::While { cond, body } => self.gen_cond_block("while", cond, body, level),
            StmtKind::ExprStmt(e) => {
                let s = self.gen_expr(e);
                // switch as a statement does not take a trailing semicolon in Zig
                if matches!(&e.kind, ExprKind::Switch { .. }) {
                    format!("{}{}\n", ind, s)
                } else {
                    format!("{}{};\n", ind, s)
                }
            }
        }
    }

    pub(super) fn gen_expr(&mut self, expr: &Expr) -> String {
        match &expr.kind {
            ExprKind::Var(name) => name.clone(),
            ExprKind::Str(s) => format!("\"{}\"", s),
            ExprKind::Int(n) => n.to_string(),
            ExprKind::Float(f) => format!("{}", f),
            ExprKind::Bool(b) => b.to_string(),
            ExprKind::Import(m) => format!("@import(\"{}\")", m),

            ExprKind::MemberAccess { obj, prop } => {
                format!("{}.{}", self.gen_expr(obj), prop)
            }

            ExprKind::Call { callee, args } => {
                if let ExprKind::MemberAccess { obj, prop } = &callee.kind {
                    if let ExprKind::Var(module) = &obj.kind {
                        if self.is_std_module(module) {
                            return self.gen_std_call(prop, args);
                        }
                    }
                    if let ExprKind::MemberAccess {
                        obj: inner_obj,
                        prop: inner_prop,
                    } = &obj.kind
                    {
                        if let ExprKind::Var(module) = &inner_obj.kind {
                            if self.is_std_module(module) {
                                return self.gen_std_ns_call(inner_prop, prop, args);
                            }
                        }
                    }
                    if let ExprKind::Var(alias) = &obj.kind {
                        if let Some((module, ns)) = self.aliases.get(alias).cloned() {
                            if module == "std" && !ns.is_empty() {
                                return self.gen_std_ns_call(&ns, prop, args);
                            }
                        }
                    }
                }
                if let ExprKind::Var(name) = &callee.kind {
                    if let Some((module, fn_name)) = self.aliases.get(name).cloned() {
                        if module == "std" {
                            return self.gen_std_call(&fn_name, args);
                        }
                    }
                }
                // Mangle calls to user-defined "main" to match the renamed fn
                let callee_s = if matches!(&callee.kind, ExprKind::Var(n) if n == "main") {
                    MAIN_MANGLED.to_string()
                } else {
                    self.gen_expr(callee)
                };
                let mut args_str = self.gen_args(args);
                // Insert __zyre_allocator as the first argument when calling an allocating fn
                if self.is_allocating_call(callee) {
                    args_str.insert(0, "__zyre_allocator".to_string());
                }
                format!("{}({})", callee_s, args_str.join(", "))
            }

            ExprKind::BinOp { op, lhs, rhs } => {
                let lhs_s = self.gen_expr(lhs);
                let rhs_s = self.gen_expr(rhs);
                let op_str = match op {
                    // Zig requires @rem for signed integer remainder (% only works on unsigned)
                    BinOp::Mod => return format!("@rem({}, {})", lhs_s, rhs_s),
                    BinOp::Add => "+",
                    BinOp::Sub => "-",
                    BinOp::Mul => "*",
                    BinOp::Div => "/",
                    BinOp::Eq => "==",
                    BinOp::NotEq => "!=",
                    BinOp::Lt => "<",
                    BinOp::Gt => ">",
                    BinOp::LtEq => "<=",
                    BinOp::GtEq => ">=",
                    BinOp::And => "and",
                    BinOp::Or => "or",
                };
                format!("({} {} {})", lhs_s, op_str, rhs_s)
            }

            ExprKind::UnOp { op, expr } => {
                let op_str = match op {
                    UnOp::Neg => "-",
                    UnOp::Not => "!",
                };
                format!("({}{})", op_str, self.gen_expr(expr))
            }

            ExprKind::ArrayLiteral(elems) => {
                let elems_str: Vec<String> = elems.iter().map(|e| self.gen_expr(e)).collect();
                format!(".{{{}}}", elems_str.join(", "))
            }

            ExprKind::Index { obj, idx } => {
                format!("{}[{}]", self.gen_expr(obj), self.gen_expr(idx))
            }

            ExprKind::Propagate(e) => format!("(try {})", self.gen_expr(e)),

            ExprKind::Catch {
                expr,
                err_name,
                body,
            } => {
                let uses_err = Self::stmts_reference_var(body, err_name);
                let capture = if uses_err {
                    self.catch_err_vars.insert(err_name.clone());
                    format!(" |{}|", err_name)
                } else {
                    String::new()
                };
                let expr_s = self.gen_expr(expr);
                let mut out = format!("{} catch{} {{\n", expr_s, capture);
                for s in body {
                    out.push_str(&self.gen_stmt(s, 1));
                }
                if uses_err {
                    self.catch_err_vars.remove(err_name);
                }
                out.push('}');
                out
            }

            ExprKind::Switch { expr, arms } => {
                let expr_s = self.gen_expr(expr);
                let mut out = format!("switch ({}) {{\n", expr_s);
                for arm in arms {
                    let pat = match &arm.pattern {
                        SwitchPattern::Ident(s) => format!(".{}", s),
                        SwitchPattern::Int(n) => n.to_string(),
                        SwitchPattern::Bool(b) => b.to_string(),
                        SwitchPattern::Else => "else".to_string(),
                    };
                    let body = match &arm.body {
                        SwitchBody::Expr(e) => {
                            format!(" => {},\n", self.gen_expr(e))
                        }
                        SwitchBody::Block(stmts) => {
                            let mut s = " => {\n".to_string();
                            for stmt in stmts {
                                s.push_str(&self.gen_stmt(stmt, 2));
                            }
                            s.push_str("    },\n");
                            s
                        }
                    };
                    out.push_str(&format!("    {}{}", pat, body));
                }
                out.push('}');
                out
            }

            ExprKind::If { cond, then, else_ } => {
                format!(
                    "if ({}) {} else {}",
                    self.gen_expr(cond),
                    self.gen_expr(then),
                    self.gen_expr(else_)
                )
            }
        }
    }

    pub(super) fn stmts_reference_var(stmts: &[Stmt], var: &str) -> bool {
        stmts.iter().any(|s| Self::stmt_references_var(s, var))
    }

    fn stmt_references_var(stmt: &Stmt, var: &str) -> bool {
        match &stmt.kind {
            StmtKind::ConstDecl { value, .. } | StmtKind::LetDecl { value, .. } => {
                Self::expr_references_var(value, var)
            }
            StmtKind::Return(Some(e)) => Self::expr_references_var(e, var),
            StmtKind::If {
                cond,
                body,
                else_body,
            } => {
                Self::expr_references_var(cond, var)
                    || Self::stmts_reference_var(body, var)
                    || else_body
                        .as_ref()
                        .is_some_and(|s| Self::stmts_reference_var(s, var))
            }
            StmtKind::While { cond, body } => {
                Self::expr_references_var(cond, var) || Self::stmts_reference_var(body, var)
            }
            StmtKind::ExprStmt(e) => Self::expr_references_var(e, var),
            _ => false,
        }
    }

    fn expr_references_var(expr: &Expr, var: &str) -> bool {
        match &expr.kind {
            ExprKind::Var(name) => name == var,
            ExprKind::Call { callee, args } => {
                Self::expr_references_var(callee, var)
                    || args.iter().any(|a| Self::expr_references_var(a, var))
            }
            ExprKind::MemberAccess { obj, .. } => Self::expr_references_var(obj, var),
            ExprKind::BinOp { lhs, rhs, .. } => {
                Self::expr_references_var(lhs, var) || Self::expr_references_var(rhs, var)
            }
            ExprKind::UnOp { expr, .. } => Self::expr_references_var(expr, var),
            ExprKind::Propagate(e) => Self::expr_references_var(e, var),
            ExprKind::Catch { expr, body, .. } => {
                Self::expr_references_var(expr, var) || Self::stmts_reference_var(body, var)
            }
            ExprKind::Switch { expr, arms } => {
                Self::expr_references_var(expr, var)
                    || arms.iter().any(|arm| match &arm.body {
                        SwitchBody::Expr(e) => Self::expr_references_var(e, var),
                        SwitchBody::Block(stmts) => Self::stmts_reference_var(stmts, var),
                    })
            }
            ExprKind::ArrayLiteral(elems) => {
                elems.iter().any(|e| Self::expr_references_var(e, var))
            }
            ExprKind::Index { obj, idx } => {
                Self::expr_references_var(obj, var) || Self::expr_references_var(idx, var)
            }
            ExprKind::If { cond, then, else_ } => {
                Self::expr_references_var(cond, var)
                    || Self::expr_references_var(then, var)
                    || Self::expr_references_var(else_, var)
            }
            _ => false,
        }
    }
}
