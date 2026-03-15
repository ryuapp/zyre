use crate::codegen::zig::ZigBackend;
use crate::parser::*;

mod fs;

impl ZigBackend {
    pub(super) fn gen_print_call(&mut self, args: &[Expr]) -> String {
        if let Some(arg) = args.first() {
            // Error variables bound in a catch block are wrapped with @errorName
            let arg_s = if let ExprKind::Var(name) = &arg.kind {
                if self.catch_err_vars.contains(name) {
                    format!("@errorName({})", name)
                } else {
                    self.gen_expr(arg)
                }
            } else {
                self.gen_expr(arg)
            };
            format!("__zyre_std_debug.print({})", arg_s)
        } else {
            "__zyre_std_debug.print(\"\")".to_string()
        }
    }

    pub(super) fn gen_std_call(&mut self, fn_name: &str, args: &[Expr]) -> String {
        let args_str = self.gen_args(args);
        format!("std.{}({})", fn_name, args_str.join(", "))
    }

    pub(super) fn gen_std_ns_call(&mut self, ns: &str, fn_name: &str, args: &[Expr]) -> String {
        match ns {
            "debug" => match fn_name {
                "print" => self.gen_print_call(args),
                _ => {
                    let args_str = self.gen_args(args);
                    format!("std.debug.{}({})", fn_name, args_str.join(", "))
                }
            },
            "fs" => self.gen_fs_call(fn_name, args),
            _ => {
                let args_str = self.gen_args(args);
                format!("std.{}.{}({})", ns, fn_name, args_str.join(", "))
            }
        }
    }

    // Check whether any calls that require an allocator are present
    pub(super) fn uses_allocator(&self, stmts: &[Stmt]) -> bool {
        stmts.iter().any(|s| self.stmt_uses_allocator(s))
    }

    fn stmt_uses_allocator(&self, stmt: &Stmt) -> bool {
        match &stmt.kind {
            StmtKind::ConstDecl { value, .. } | StmtKind::LetDecl { value, .. } => {
                self.expr_uses_allocator(value)
            }
            StmtKind::Return(Some(e)) => self.expr_uses_allocator(e),
            StmtKind::If {
                cond,
                body,
                else_body,
            } => {
                self.expr_uses_allocator(cond)
                    || self.uses_allocator(body)
                    || else_body.as_ref().is_some_and(|s| self.uses_allocator(s))
            }
            StmtKind::While { cond, body } => {
                self.expr_uses_allocator(cond) || self.uses_allocator(body)
            }
            StmtKind::ExprStmt(e) => self.expr_uses_allocator(e),
            _ => false,
        }
    }

    fn expr_uses_allocator(&self, expr: &Expr) -> bool {
        // Query each namespace module
        if self.fs_expr_uses_allocator(expr) {
            return true;
        }
        // Check recursively
        match &expr.kind {
            ExprKind::Call { callee, args } => {
                // Calling a user function that transitively needs an allocator
                if self.is_allocating_call(callee) {
                    return true;
                }
                self.expr_uses_allocator(callee) || args.iter().any(|a| self.expr_uses_allocator(a))
            }
            ExprKind::MemberAccess { obj, .. } => self.expr_uses_allocator(obj),
            ExprKind::BinOp { lhs, rhs, .. } => {
                self.expr_uses_allocator(lhs) || self.expr_uses_allocator(rhs)
            }
            ExprKind::UnOp { expr, .. } => self.expr_uses_allocator(expr),
            ExprKind::Propagate(e) => self.expr_uses_allocator(e),
            ExprKind::Catch { expr, body, .. } => {
                self.expr_uses_allocator(expr) || self.uses_allocator(body)
            }
            ExprKind::Switch { expr, arms } => {
                self.expr_uses_allocator(expr)
                    || arms.iter().any(|arm| match &arm.body {
                        SwitchBody::Expr(e) => self.expr_uses_allocator(e),
                        SwitchBody::Block(stmts) => self.uses_allocator(stmts),
                    })
            }
            ExprKind::ArrayLiteral(elems) => elems.iter().any(|e| self.expr_uses_allocator(e)),
            ExprKind::Index { obj, idx } => {
                self.expr_uses_allocator(obj) || self.expr_uses_allocator(idx)
            }
            ExprKind::If { cond, then, else_ } => {
                self.expr_uses_allocator(cond)
                    || self.expr_uses_allocator(then)
                    || self.expr_uses_allocator(else_)
            }
            _ => false,
        }
    }
}
