use crate::lexer::{self, Span};
use crate::parser::{self, *};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct ZyError {
    pub message: String,
    pub span: Option<Span>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Ty {
    I32,
    F64,
    String,
    Bool,
    Void,
    Array(Box<Ty>, u64),
    Optional(Box<Ty>),
    ErrorUnion(Box<Ty>),
    Struct(String),
    Enum(String),
    Fn { params: Vec<Ty>, ret: Box<Ty> },
    Module(String),
    Unknown,
}

impl Ty {
    fn from_type_expr(te: &TypeExpr, types: &HashMap<String, Ty>) -> Self {
        match te {
            TypeExpr::Name(name) => match name.as_str() {
                "i32" => Ty::I32,
                "f64" => Ty::F64,
                "string" => Ty::String,
                "bool" => Ty::Bool,
                "void" => Ty::Void,
                other => types.get(other).cloned().unwrap_or(Ty::Unknown),
            },
            TypeExpr::Array(inner, n) => Ty::Array(Box::new(Ty::from_type_expr(inner, types)), *n),
            TypeExpr::Optional(inner) => Ty::Optional(Box::new(Ty::from_type_expr(inner, types))),
            TypeExpr::ErrorUnion(_, inner) => {
                Ty::ErrorUnion(Box::new(Ty::from_type_expr(inner, types)))
            }
        }
    }

    fn is_assignable_from(&self, other: &Ty) -> bool {
        if self == other {
            return true;
        }
        // ?T accepts both T and null
        if let Ty::Optional(inner) = self {
            return inner.as_ref() == other || other == &Ty::Void;
        }
        // E!T also accepts T
        if let Ty::ErrorUnion(inner) = self {
            return inner.as_ref() == other;
        }
        false
    }

    fn display(&self) -> String {
        match self {
            Ty::I32 => "i32".to_string(),
            Ty::F64 => "f64".to_string(),
            Ty::String => "string".to_string(),
            Ty::Bool => "bool".to_string(),
            Ty::Void => "void".to_string(),
            Ty::Array(t, n) => format!("{}[{}]", t.display(), n),
            Ty::Optional(t) => format!("?{}", t.display()),
            Ty::ErrorUnion(t) => format!("!{}", t.display()),
            Ty::Struct(name) => name.clone(),
            Ty::Enum(name) => name.clone(),
            Ty::Fn { .. } => "fn".to_string(),
            Ty::Module(name) => format!("module({})", name),
            Ty::Unknown => "unknown".to_string(),
        }
    }
}

pub struct TypeChecker {
    // Top-level symbol table (functions, types, imports)
    globals: HashMap<String, Ty>,
    // User-defined types (struct / enum)
    types: HashMap<String, Ty>,
    // Struct field types: struct name -> [(field name, type)]
    struct_fields: HashMap<String, Vec<(String, Ty)>>,
    // Exports of local .zy modules: module_path -> (fn name -> type)
    module_exports: HashMap<String, HashMap<String, Ty>>,
    // Directory of the source file (used for resolving local imports)
    source_dir: PathBuf,
    pub errors: Vec<ZyError>,
    // Span currently being checked
    current_span: Span,
}

impl TypeChecker {
    pub fn new(source_dir: &Path) -> Self {
        TypeChecker {
            globals: HashMap::new(),
            types: HashMap::new(),
            struct_fields: HashMap::new(),
            module_exports: HashMap::new(),
            source_dir: source_dir.to_path_buf(),
            errors: Vec::new(),
            current_span: (0, 0),
        }
    }

    /// Parses a local .zy file and extracts the types of exported functions
    fn extract_module_exports(&self, path: &str) -> HashMap<String, Ty> {
        let rel = path.trim_start_matches("./");
        let file_path = self.source_dir.join(rel);
        let source = match std::fs::read_to_string(&file_path) {
            Ok(s) => s,
            Err(_) => return HashMap::new(),
        };
        let tokens = lexer::tokenize(&source);
        let (ast, _, _) = parser::parse(tokens);
        let mut exports = HashMap::new();
        for item in &ast {
            if let TopLevel::FnDecl(f) = item {
                if f.exported {
                    let params: Vec<Ty> = f
                        .params
                        .iter()
                        .map(|(_, ty)| Ty::from_type_expr(ty, &self.types))
                        .collect();
                    let ret = Ty::from_type_expr(&f.ret, &self.types);
                    exports.insert(
                        f.name.clone(),
                        Ty::Fn {
                            params,
                            ret: Box::new(ret),
                        },
                    );
                }
            }
        }
        exports
    }

    fn error(&mut self, msg: String) {
        self.errors.push(ZyError {
            span: if self.current_span != (0, 0) {
                Some(self.current_span)
            } else {
                None
            },
            message: msg,
        });
    }

    pub fn check(&mut self, program: &Program) {
        // Pass 1: collect top-level symbols (import / alias / fn / struct / enum only)
        let mut import_names: std::collections::HashSet<String> = std::collections::HashSet::new();
        for item in program {
            match item {
                TopLevel::ConstDecl { name, value, .. } => {
                    let is_import = matches!(&value.kind, ExprKind::Import(_));
                    // Register all top-level ConstDecls into globals in order
                    let ty = self.check_expr(value, &self.globals.clone());
                    self.globals.insert(name.clone(), ty);
                    if is_import {
                        import_names.insert(name.clone());
                        // Pre-collect exports of local .zy modules
                        if let ExprKind::Import(path) = &value.kind {
                            if path.ends_with(".zy") {
                                let exports = self.extract_module_exports(path);
                                self.module_exports.insert(path.clone(), exports);
                            }
                        }
                    }
                }
                TopLevel::FnDecl(f) => {
                    let params: Vec<Ty> = f
                        .params
                        .iter()
                        .map(|(_, ty)| Ty::from_type_expr(ty, &self.types))
                        .collect();
                    let ret = Ty::from_type_expr(&f.ret, &self.types);
                    self.globals.insert(
                        f.name.clone(),
                        Ty::Fn {
                            params,
                            ret: Box::new(ret),
                        },
                    );
                }
                TopLevel::StructDecl { name, fields, .. } => {
                    let field_tys: Vec<(String, Ty)> = fields
                        .iter()
                        .map(|f| (f.name.clone(), Ty::from_type_expr(&f.ty, &self.types)))
                        .collect();
                    self.struct_fields.insert(name.clone(), field_tys);
                    self.types.insert(name.clone(), Ty::Struct(name.clone()));
                    self.globals.insert(name.clone(), Ty::Struct(name.clone()));
                }
                TopLevel::EnumDecl { name, .. } => {
                    self.types.insert(name.clone(), Ty::Enum(name.clone()));
                    self.globals.insert(name.clone(), Ty::Enum(name.clone()));
                }
                TopLevel::Stmt(_) => {}
            }
        }

        // Pass 2: check function bodies + top-level body statements (in order)
        let mut body_scope = self.globals.clone();
        for item in program {
            match item {
                TopLevel::FnDecl(f) => {
                    let ret_ty = Ty::from_type_expr(&f.ret, &self.types);
                    let mut scope = self.globals.clone();
                    for (pname, pty) in &f.params {
                        scope.insert(pname.clone(), Ty::from_type_expr(pty, &self.types));
                    }
                    self.check_block(&f.body, &mut scope, &ret_ty);
                }
                TopLevel::ConstDecl { name, value, .. } => {
                    // Only runtime const decls (import/alias were already handled in pass 1)
                    if !self.globals.contains_key(name) {
                        let ty = self.check_expr(value, &body_scope.clone());
                        body_scope.insert(name.clone(), ty);
                    }
                }
                TopLevel::Stmt(stmt) => {
                    self.check_stmt(stmt, &mut body_scope, &Ty::Void);
                }
                _ => {}
            }
        }

        // Pass 3: unused-check for top-level variable aliases (const a = some_var)
        let alias_decls: Vec<(String, Span)> = program
            .iter()
            .filter_map(|item| {
                if let TopLevel::ConstDecl {
                    name,
                    name_span,
                    value,
                    ..
                } = item
                {
                    if !name.starts_with('_') {
                        if let ExprKind::Var(_) = &value.kind {
                            return Some((name.clone(), *name_span));
                        }
                    }
                }
                None
            })
            .collect();

        if !alias_decls.is_empty() {
            let mut all_refs = std::collections::HashSet::new();
            for item in program {
                match item {
                    TopLevel::FnDecl(f) => {
                        for stmt in &f.body {
                            Self::refs_stmt(stmt, &mut all_refs);
                        }
                    }
                    TopLevel::Stmt(stmt) => {
                        Self::refs_stmt(stmt, &mut all_refs);
                    }
                    TopLevel::ConstDecl { value, .. } => {
                        Self::refs_expr(value, &mut all_refs);
                    }
                    _ => {}
                }
            }
            for (name, span) in alias_decls {
                if !all_refs.contains(&name) {
                    self.current_span = span;
                    self.error(format!("Unused top-level alias: '{}'", name));
                }
            }
        }
    }

    fn check_block(&mut self, stmts: &[Stmt], scope: &mut HashMap<String, Ty>, ret_ty: &Ty) {
        // Collect all variable references in the block upfront
        let refs = Self::collect_refs(stmts);

        // Record local declarations (names starting with _ are excluded as intentionally unused)
        let mut local_decls: Vec<(String, Span)> = Vec::new();
        for stmt in stmts {
            match &stmt.kind {
                StmtKind::ConstDecl { name, .. } | StmtKind::LetDecl { name, .. } => {
                    if !name.starts_with('_') {
                        local_decls.push((name.clone(), stmt.span));
                    }
                }
                _ => {}
            }
            self.check_stmt(stmt, scope, ret_ty);
        }

        for (name, span) in local_decls {
            if !refs.contains(&name) {
                self.current_span = span;
                self.error(format!("Unused local constant: '{}'", name));
            }
        }
    }

    fn collect_refs(stmts: &[Stmt]) -> std::collections::HashSet<String> {
        let mut refs = std::collections::HashSet::new();
        Self::refs_stmts(stmts, &mut refs);
        refs
    }

    fn refs_stmts(stmts: &[Stmt], refs: &mut std::collections::HashSet<String>) {
        for stmt in stmts {
            Self::refs_stmt(stmt, refs);
        }
    }

    fn refs_stmt(stmt: &Stmt, refs: &mut std::collections::HashSet<String>) {
        match &stmt.kind {
            StmtKind::ConstDecl { value, .. } | StmtKind::LetDecl { value, .. } => {
                Self::refs_expr(value, refs);
            }
            StmtKind::Return(Some(e)) => Self::refs_expr(e, refs),
            StmtKind::ExprStmt(e) => Self::refs_expr(e, refs),
            StmtKind::If {
                cond,
                body,
                else_body,
            } => {
                Self::refs_expr(cond, refs);
                Self::refs_stmts(body, refs);
                if let Some(else_stmts) = else_body {
                    Self::refs_stmts(else_stmts, refs);
                }
            }
            StmtKind::While { cond, body } => {
                Self::refs_expr(cond, refs);
                Self::refs_stmts(body, refs);
            }
            _ => {}
        }
    }

    fn refs_expr(expr: &Expr, refs: &mut std::collections::HashSet<String>) {
        match &expr.kind {
            ExprKind::Var(name) => {
                refs.insert(name.clone());
            }
            ExprKind::Call { callee, args } => {
                Self::refs_expr(callee, refs);
                for a in args {
                    Self::refs_expr(a, refs);
                }
            }
            ExprKind::MemberAccess { obj, .. } => Self::refs_expr(obj, refs),
            ExprKind::BinOp { lhs, rhs, .. } => {
                Self::refs_expr(lhs, refs);
                Self::refs_expr(rhs, refs);
            }
            ExprKind::UnOp { expr, .. } | ExprKind::Propagate(expr) => Self::refs_expr(expr, refs),
            ExprKind::Catch { expr, body, .. } => {
                Self::refs_expr(expr, refs);
                Self::refs_stmts(body, refs);
            }
            ExprKind::Switch { expr, arms } => {
                Self::refs_expr(expr, refs);
                for arm in arms {
                    match &arm.body {
                        SwitchBody::Expr(e) => Self::refs_expr(e, refs),
                        SwitchBody::Block(stmts) => Self::refs_stmts(stmts, refs),
                    }
                }
            }
            ExprKind::ArrayLiteral(elems) => {
                for e in elems {
                    Self::refs_expr(e, refs);
                }
            }
            ExprKind::Index { obj, idx } => {
                Self::refs_expr(obj, refs);
                Self::refs_expr(idx, refs);
            }
            ExprKind::If { cond, then, else_ } => {
                Self::refs_expr(cond, refs);
                Self::refs_expr(then, refs);
                Self::refs_expr(else_, refs);
            }
            _ => {}
        }
    }

    fn check_stmt(&mut self, stmt: &Stmt, scope: &mut HashMap<String, Ty>, ret_ty: &Ty) {
        self.current_span = stmt.span;
        match &stmt.kind {
            StmtKind::ConstDecl { name, ty, value } | StmtKind::LetDecl { name, ty, value } => {
                let val_ty = self.check_expr(value, scope);
                let decl_ty = if let Some(t) = ty {
                    Ty::from_type_expr(t, &self.types)
                } else {
                    val_ty.clone()
                };
                if !decl_ty.is_assignable_from(&val_ty) {
                    self.error(format!(
                        "Type mismatch: '{}' is declared as '{}' but got '{}'",
                        name,
                        decl_ty.display(),
                        val_ty.display()
                    ));
                }
                scope.insert(name.clone(), decl_ty);
            }
            StmtKind::Return(None) => {
                if ret_ty != &Ty::Void {
                    self.error(format!(
                        "Expected return value of type '{}', got nothing",
                        ret_ty.display()
                    ));
                }
            }
            StmtKind::Return(Some(expr)) => {
                let ty = self.check_expr(expr, scope);
                if !ret_ty.is_assignable_from(&ty) {
                    self.error(format!(
                        "Return type mismatch: expected '{}', got '{}'",
                        ret_ty.display(),
                        ty.display()
                    ));
                }
            }
            StmtKind::If {
                cond,
                body,
                else_body,
            } => {
                let cond_ty = self.check_expr(cond, scope);
                if cond_ty != Ty::Bool {
                    self.error(format!(
                        "if condition must be bool, got '{}'",
                        cond_ty.display()
                    ));
                }
                let mut inner = scope.clone();
                self.check_block(body, &mut inner, ret_ty);
                if let Some(else_stmts) = else_body {
                    let mut inner = scope.clone();
                    self.check_block(else_stmts, &mut inner, ret_ty);
                }
            }
            StmtKind::While { cond, body } => {
                let cond_ty = self.check_expr(cond, scope);
                if cond_ty != Ty::Bool {
                    self.error(format!(
                        "while condition must be bool, got '{}'",
                        cond_ty.display()
                    ));
                }
                let mut inner = scope.clone();
                self.check_block(body, &mut inner, ret_ty);
            }
            StmtKind::Break | StmtKind::Continue => {}
            StmtKind::ExprStmt(expr) => {
                self.check_expr(expr, scope);
            }
        }
    }

    fn check_fn_args(&mut self, params: &[Ty], arg_tys: &[(Ty, Span)], call_span: Span) {
        if arg_tys.len() != params.len() {
            self.current_span = call_span;
            self.error(format!(
                "Expected {} argument(s), got {}",
                params.len(),
                arg_tys.len()
            ));
        }
        for (i, ((arg_ty, arg_span), param_ty)) in arg_tys.iter().zip(params.iter()).enumerate() {
            if !param_ty.is_assignable_from(arg_ty) {
                self.current_span = *arg_span;
                self.error(format!(
                    "Argument {} type mismatch: expected '{}', got '{}'",
                    i + 1,
                    param_ty.display(),
                    arg_ty.display()
                ));
            }
        }
    }

    fn check_expr(&mut self, expr: &Expr, scope: &HashMap<String, Ty>) -> Ty {
        // Update to expression-level span (points to the exact location, e.g. arguments)
        self.current_span = expr.span;
        match &expr.kind {
            ExprKind::Int(_) => Ty::I32,
            ExprKind::Float(_) => Ty::F64,
            ExprKind::Str(_) => Ty::String,
            ExprKind::Bool(_) => Ty::Bool,
            ExprKind::Import(m) => Ty::Module(m.clone()),

            ExprKind::Var(name) => {
                if let Some(ty) = scope.get(name) {
                    ty.clone()
                } else {
                    self.error(format!("Undefined variable: '{}'", name));
                    Ty::Unknown
                }
            }

            ExprKind::MemberAccess { obj, prop } => {
                let obj_ty = self.check_expr(obj, scope);
                match &obj_ty {
                    Ty::Module(module) => self.check_std_member(module, prop),
                    Ty::Struct(name) => {
                        let name = name.clone();
                        if let Some(fields) = self.struct_fields.get(&name) {
                            if let Some((_, ty)) = fields.iter().find(|(n, _)| n == prop) {
                                ty.clone()
                            } else {
                                self.error(format!("Struct '{}' has no field '{}'", name, prop));
                                Ty::Unknown
                            }
                        } else {
                            Ty::Unknown
                        }
                    }
                    _ => Ty::Unknown,
                }
            }

            ExprKind::Call { callee, args } => {
                // Special handling for std.debug.print and similar calls
                if let ExprKind::MemberAccess { obj, prop } = &callee.kind {
                    if let ExprKind::Var(var_name) = &obj.kind {
                        if let Some(Ty::Module(module_path)) = scope.get(var_name).cloned() {
                            // Call to a local .zy module
                            if module_path.ends_with(".zy") {
                                let fn_ty = self
                                    .module_exports
                                    .get(&module_path)
                                    .and_then(|e| e.get(prop))
                                    .cloned();
                                let arg_tys: Vec<(Ty, Span)> = args
                                    .iter()
                                    .map(|a| {
                                        let ty = self.check_expr(a, scope);
                                        (ty, a.span)
                                    })
                                    .collect();
                                return if let Some(Ty::Fn { params, ret }) = fn_ty {
                                    self.check_fn_args(&params, &arg_tys, expr.span);
                                    *ret
                                } else {
                                    Ty::Unknown
                                };
                            }
                            for arg in args {
                                self.check_expr(arg, scope);
                            }
                            // Sub-module alias (e.g. const fs = std.fs; fs.readTextFile(...))
                            if let Some(dot) = module_path.find('.') {
                                let root = module_path[..dot].to_string();
                                let ns = module_path[dot + 1..].to_string();
                                return self.check_std_ns_call(&root, &ns, prop, args);
                            } else {
                                return Ty::Unknown;
                            }
                        }
                    }
                    // Nested calls such as std.fs.readTextFile
                    if let ExprKind::MemberAccess {
                        obj: inner_obj,
                        prop: inner_prop,
                    } = &obj.kind
                    {
                        if let ExprKind::Var(module) = &inner_obj.kind {
                            if matches!(scope.get(module), Some(Ty::Module(_))) {
                                for arg in args {
                                    self.check_expr(arg, scope);
                                }
                                return self.check_std_ns_call(module, inner_prop, prop, args);
                            }
                        }
                    }
                }

                let callee_ty = self.check_expr(callee, scope);
                let arg_tys: Vec<(Ty, Span)> = args
                    .iter()
                    .map(|a| {
                        let ty = self.check_expr(a, scope);
                        (ty, a.span)
                    })
                    .collect();
                match callee_ty {
                    Ty::Fn { params, ret } => {
                        self.check_fn_args(&params, &arg_tys, expr.span);
                        *ret
                    }
                    Ty::Unknown => Ty::Unknown,
                    other => {
                        self.error(format!("'{}' is not callable", other.display()));
                        Ty::Unknown
                    }
                }
            }

            ExprKind::BinOp { op, lhs, rhs } => {
                let lhs_ty = self.check_expr(lhs, scope);
                let rhs_ty = self.check_expr(rhs, scope);
                match op {
                    BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Mod => {
                        if lhs_ty != rhs_ty {
                            self.error(format!(
                                "Binary op type mismatch: '{}' vs '{}'",
                                lhs_ty.display(),
                                rhs_ty.display()
                            ));
                        }
                        lhs_ty
                    }
                    BinOp::Eq
                    | BinOp::NotEq
                    | BinOp::Lt
                    | BinOp::Gt
                    | BinOp::LtEq
                    | BinOp::GtEq => Ty::Bool,
                    BinOp::And | BinOp::Or => {
                        if lhs_ty != Ty::Bool || rhs_ty != Ty::Bool {
                            self.error("&& and || require bool operands".to_string());
                        }
                        Ty::Bool
                    }
                }
            }

            ExprKind::UnOp { op, expr } => {
                let ty = self.check_expr(expr, scope);
                match op {
                    UnOp::Neg => ty,
                    UnOp::Not => {
                        if ty != Ty::Bool {
                            self.error(format!("! requires bool, got '{}'", ty.display()));
                        }
                        Ty::Bool
                    }
                }
            }

            ExprKind::ArrayLiteral(elems) => {
                if elems.is_empty() {
                    return Ty::Array(Box::new(Ty::Unknown), 0);
                }
                let elem_ty = self.check_expr(&elems[0], scope);
                for e in elems.iter().skip(1) {
                    let t = self.check_expr(e, scope);
                    if t != elem_ty {
                        self.error(format!(
                            "Array element type mismatch: expected '{}', got '{}'",
                            elem_ty.display(),
                            t.display()
                        ));
                    }
                }
                Ty::Array(Box::new(elem_ty), elems.len() as u64)
            }

            ExprKind::Index { obj, idx } => {
                let obj_ty = self.check_expr(obj, scope);
                let idx_ty = self.check_expr(idx, scope);
                if idx_ty != Ty::I32 {
                    self.error(format!(
                        "Array index must be i32, got '{}'",
                        idx_ty.display()
                    ));
                }
                match obj_ty {
                    Ty::Array(elem_ty, _) => *elem_ty,
                    other => {
                        self.error(format!("Cannot index into '{}'", other.display()));
                        Ty::Unknown
                    }
                }
            }

            ExprKind::Propagate(expr) => {
                let ty = self.check_expr(expr, scope);
                match ty {
                    Ty::Optional(inner) | Ty::ErrorUnion(inner) => *inner,
                    _ => ty,
                }
            }

            ExprKind::Catch {
                expr,
                err_name,
                body,
            } => {
                let ty = self.check_expr(expr, scope);
                let mut inner = scope.clone();
                inner.insert(err_name.clone(), Ty::String);
                self.check_block(body, &mut inner, &Ty::Void);
                match ty {
                    Ty::ErrorUnion(inner_ty) => *inner_ty,
                    other => other,
                }
            }

            ExprKind::Switch { expr, arms } => {
                self.check_expr(expr, scope);
                let mut result_ty = Ty::Unknown;
                for arm in arms {
                    let arm_ty = match &arm.body {
                        SwitchBody::Expr(e) => self.check_expr(e, scope),
                        SwitchBody::Block(stmts) => {
                            let mut inner = scope.clone();
                            self.check_block(stmts, &mut inner, &Ty::Void);
                            Ty::Void
                        }
                    };
                    if result_ty == Ty::Unknown {
                        result_ty = arm_ty;
                    }
                }
                result_ty
            }

            ExprKind::If { cond, then, else_ } => {
                let cond_ty = self.check_expr(cond, scope);
                if cond_ty != Ty::Bool && cond_ty != Ty::Unknown {
                    self.error("if condition must be bool".to_string());
                }
                let then_ty = self.check_expr(then, scope);
                let else_ty = self.check_expr(else_, scope);
                if then_ty != else_ty && then_ty != Ty::Unknown && else_ty != Ty::Unknown {
                    self.error(format!(
                        "if branches have different types: '{}' vs '{}'",
                        then_ty.display(),
                        else_ty.display()
                    ));
                }
                then_ty
            }
        }
    }

    fn check_std_member(&self, _module: &str, prop: &str) -> Ty {
        match prop {
            "fs" => Ty::Module("std.fs".to_string()),
            "debug" => Ty::Module("std.debug".to_string()),
            _ => Ty::Unknown,
        }
    }

    fn check_std_ns_call(&mut self, module: &str, ns: &str, fn_name: &str, args: &[Expr]) -> Ty {
        match (module, ns, fn_name) {
            ("std", "debug", "print") => {
                if args.len() != 1 {
                    self.error(format!(
                        "std.debug.print expects 1 argument, got {}",
                        args.len()
                    ));
                }
                Ty::Void
            }
            ("std", "fs", "readTextFile") => {
                if args.len() != 1 {
                    self.error(format!(
                        "std.fs.readTextFile expects 1 argument, got {}",
                        args.len()
                    ));
                }
                Ty::ErrorUnion(Box::new(Ty::String))
            }
            _ => Ty::Unknown,
        }
    }
}

/// Test-compatible interface (returns only error messages)
#[cfg(test)]
pub fn check(program: &Program, source_dir: &Path) -> Vec<String> {
    let mut tc = TypeChecker::new(source_dir);
    tc.check(program);
    tc.errors.into_iter().map(|e| e.message).collect()
}

/// Returns errors and warnings with span information
pub fn check_with_diagnostics(program: &Program, source_dir: &Path) -> Vec<ZyError> {
    let mut tc = TypeChecker::new(source_dir);
    tc.check(program);
    tc.errors
}
