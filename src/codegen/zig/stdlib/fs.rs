use crate::codegen::zig::ZigBackend;
use crate::parser::*;

impl ZigBackend {
    pub(super) fn gen_fs_call(&mut self, fn_name: &str, args: &[Expr]) -> String {
        match fn_name {
            "readTextFile" => {
                if let Some(arg) = args.first() {
                    format!(
                        "std.fs.cwd().readFileAlloc(__zyre_allocator, {}, std.math.maxInt(usize))",
                        self.gen_expr(arg)
                    )
                } else {
                    "std.fs.cwd().readFileAlloc(__zyre_allocator, \"\", std.math.maxInt(usize))"
                        .to_string()
                }
            }
            _ => {
                let args_str = self.gen_args(args);
                format!("std.fs.{}({})", fn_name, args_str.join(", "))
            }
        }
    }

    pub(super) fn fs_expr_uses_allocator(&self, expr: &Expr) -> bool {
        match &expr.kind {
            ExprKind::Call { callee, .. } => {
                if let ExprKind::MemberAccess { obj, prop } = &callee.kind {
                    // std.fs.readTextFile(...)
                    if let ExprKind::MemberAccess {
                        obj: inner,
                        prop: ns,
                    } = &obj.kind
                    {
                        if let ExprKind::Var(module) = &inner.kind {
                            if self.is_std_module(module) && ns == "fs" && prop == "readTextFile" {
                                return true;
                            }
                        }
                    }
                    // Via alias: const fs = std.fs; fs.readTextFile(...)
                    if let ExprKind::Var(alias) = &obj.kind {
                        if let Some((module, ns)) = self.aliases.get(alias) {
                            if module == "std" && ns == "fs" && prop == "readTextFile" {
                                return true;
                            }
                        }
                    }
                }
                false
            }
            _ => false,
        }
    }
}
