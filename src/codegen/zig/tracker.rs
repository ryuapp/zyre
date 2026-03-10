use super::ZigBackend;
use crate::parser::*;
use std::collections::{HashMap, HashSet};

impl ZigBackend {
    /// Transitively collects names of functions that require an allocator
    pub(super) fn collect_allocating_fns(&self, program: &Program) -> HashSet<String> {
        let fn_bodies: HashMap<String, &[Stmt]> = program
            .iter()
            .filter_map(|item| {
                if let TopLevel::FnDecl(f) = item {
                    Some((f.name.clone(), f.body.as_slice()))
                } else {
                    None
                }
            })
            .collect();

        // Collect functions that directly use an allocator
        let mut allocating: HashSet<String> = fn_bodies
            .iter()
            .filter(|(_, stmts)| self.uses_allocator(stmts))
            .map(|(name, _)| name.clone())
            .collect();

        // Propagate transitively
        loop {
            let mut changed = false;
            for (name, stmts) in &fn_bodies {
                if allocating.contains(name) {
                    continue;
                }
                if self.stmts_call_any(stmts, &allocating) {
                    allocating.insert(name.clone());
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }

        allocating
    }

    pub(super) fn stmts_call_any(&self, stmts: &[Stmt], targets: &HashSet<String>) -> bool {
        stmts.iter().any(|s| self.stmt_calls_any(s, targets))
    }

    fn stmt_calls_any(&self, stmt: &Stmt, targets: &HashSet<String>) -> bool {
        match &stmt.kind {
            StmtKind::ConstDecl { value, .. } | StmtKind::LetDecl { value, .. } => {
                self.expr_calls_any(value, targets)
            }
            StmtKind::Return(Some(e)) => self.expr_calls_any(e, targets),
            StmtKind::ExprStmt(e) => self.expr_calls_any(e, targets),
            StmtKind::If {
                cond,
                body,
                else_body,
            } => {
                self.expr_calls_any(cond, targets)
                    || self.stmts_call_any(body, targets)
                    || else_body
                        .as_ref()
                        .is_some_and(|s| self.stmts_call_any(s, targets))
            }
            StmtKind::While { cond, body } => {
                self.expr_calls_any(cond, targets) || self.stmts_call_any(body, targets)
            }
            _ => false,
        }
    }

    pub(super) fn expr_calls_any(&self, expr: &Expr, targets: &HashSet<String>) -> bool {
        match &expr.kind {
            ExprKind::Call { callee, args } => {
                if let ExprKind::Var(name) = &callee.kind {
                    if targets.contains(name) {
                        return true;
                    }
                }
                self.expr_calls_any(callee, targets)
                    || args.iter().any(|a| self.expr_calls_any(a, targets))
            }
            ExprKind::MemberAccess { obj, .. } => self.expr_calls_any(obj, targets),
            ExprKind::BinOp { lhs, rhs, .. } => {
                self.expr_calls_any(lhs, targets) || self.expr_calls_any(rhs, targets)
            }
            ExprKind::UnOp { expr, .. } | ExprKind::Propagate(expr) => {
                self.expr_calls_any(expr, targets)
            }
            ExprKind::Catch { expr, body, .. } => {
                self.expr_calls_any(expr, targets) || self.stmts_call_any(body, targets)
            }
            ExprKind::Switch { expr, arms } => {
                self.expr_calls_any(expr, targets)
                    || arms.iter().any(|arm| match &arm.body {
                        SwitchBody::Expr(e) => self.expr_calls_any(e, targets),
                        SwitchBody::Block(stmts) => self.stmts_call_any(stmts, targets),
                    })
            }
            ExprKind::If { cond, then, else_ } => {
                self.expr_calls_any(cond, targets)
                    || self.expr_calls_any(then, targets)
                    || self.expr_calls_any(else_, targets)
            }
            _ => false,
        }
    }

    /// Checks whether std is used anywhere in the program
    pub(super) fn program_uses_std(&self, program: &Program) -> bool {
        program.iter().any(|item| match item {
            TopLevel::FnDecl(f) => self.stmts_use_std(&f.body),
            TopLevel::Stmt(s) => self.stmt_uses_std(s),
            TopLevel::ConstDecl { value, .. } => self.expr_uses_std(value),
            _ => false,
        })
    }

    fn stmts_use_std(&self, stmts: &[Stmt]) -> bool {
        stmts.iter().any(|s| self.stmt_uses_std(s))
    }

    fn stmt_uses_std(&self, stmt: &Stmt) -> bool {
        match &stmt.kind {
            StmtKind::ConstDecl { value, .. } | StmtKind::LetDecl { value, .. } => {
                self.expr_uses_std(value)
            }
            StmtKind::Return(Some(e)) => self.expr_uses_std(e),
            StmtKind::If {
                cond,
                body,
                else_body,
            } => {
                self.expr_uses_std(cond)
                    || self.stmts_use_std(body)
                    || else_body.as_ref().is_some_and(|s| self.stmts_use_std(s))
            }
            StmtKind::While { cond, body } => self.expr_uses_std(cond) || self.stmts_use_std(body),
            StmtKind::ExprStmt(e) => self.expr_uses_std(e),
            _ => false,
        }
    }

    fn expr_uses_std(&self, expr: &Expr) -> bool {
        match &expr.kind {
            ExprKind::Call { callee, args } => {
                if let ExprKind::MemberAccess { obj, .. } = &callee.kind {
                    if let ExprKind::Var(m) = &obj.kind {
                        if self.is_std_module(m) {
                            return true;
                        }
                        if self
                            .aliases
                            .get(m)
                            .map(|(r, _)| r == "std")
                            .unwrap_or(false)
                        {
                            return true;
                        }
                    }
                    if let ExprKind::MemberAccess { obj: inner, .. } = &obj.kind {
                        if let ExprKind::Var(m) = &inner.kind {
                            if self.is_std_module(m) {
                                return true;
                            }
                        }
                    }
                }
                self.expr_uses_std(callee) || args.iter().any(|a| self.expr_uses_std(a))
            }
            ExprKind::MemberAccess { obj, .. } => self.expr_uses_std(obj),
            ExprKind::BinOp { lhs, rhs, .. } => self.expr_uses_std(lhs) || self.expr_uses_std(rhs),
            ExprKind::UnOp { expr, .. } => self.expr_uses_std(expr),
            ExprKind::Propagate(e) => self.expr_uses_std(e),
            ExprKind::Catch { expr, body, .. } => {
                self.expr_uses_std(expr) || self.stmts_use_std(body)
            }
            ExprKind::Switch { expr, arms } => {
                self.expr_uses_std(expr)
                    || arms.iter().any(|arm| match &arm.body {
                        SwitchBody::Expr(e) => self.expr_uses_std(e),
                        SwitchBody::Block(stmts) => self.stmts_use_std(stmts),
                    })
            }
            ExprKind::ArrayLiteral(elems) => elems.iter().any(|e| self.expr_uses_std(e)),
            ExprKind::Index { obj, idx } => self.expr_uses_std(obj) || self.expr_uses_std(idx),
            ExprKind::If { cond, then, else_ } => {
                self.expr_uses_std(cond) || self.expr_uses_std(then) || self.expr_uses_std(else_)
            }
            _ => false,
        }
    }
}
