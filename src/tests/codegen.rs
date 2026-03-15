use super::compile;

#[test]
fn test_codegen_user_import() {
    let out = compile(
        r#"
        const math = import("./modules/math.zy");
    "#,
    );
    assert!(
        out.contains("const math = @import(\"./modules/math.zig\")"),
        "got:\n{}",
        out
    );
}

#[test]
fn test_codegen_no_std_when_unused() {
    let out = compile(
        r#"
        export fn add(a: i32, b: i32): i32 {
            return a + b;
        }
    "#,
    );
    assert!(!out.contains("const std"), "expected no std, got:\n{}", out);
}

#[test]
fn test_codegen_std_when_used() {
    let out = compile(
        r#"
        const std = import("std");
        std.debug.print("hello");
    "#,
    );
    assert!(
        out.contains("const std = @import(\"std\")"),
        "got:\n{}",
        out
    );
}

#[test]
fn test_codegen_print_generic_format() {
    let out = compile(
        r#"
        const std = import("std");
        std.debug.print("hello");
    "#,
    );
    assert!(
        out.contains("__zyre_std_debug.print("),
        "expected __zyre_std_debug.print call, got:\n{}",
        out
    );
    assert!(
        out.contains("@import(\"zyre_std_debug.zig\")"),
        "expected zyre_std_debug import, got:\n{}",
        out
    );
}

#[test]
fn test_codegen_export_fn() {
    let out = compile(
        r#"
        export fn add(a: i32, b: i32): i32 {
            return a + b;
        }
    "#,
    );
    assert!(out.contains("pub fn add"), "got:\n{}", out);
}

#[test]
fn test_codegen_import_alias() {
    let out = compile(
        r#"
        const s = import("std");
        s.debug.print("hello");
    "#,
    );
    assert!(out.contains("__zyre_std_debug.print"), "got:\n{}", out);
    assert!(!out.contains("const s = @import"), "got:\n{}", out);
}

#[test]
fn test_codegen_switch_int() {
    let out = compile(
        r#"
        const std = import("std");
        fn check(n: i32): void {
            switch n {
                1 => std.debug.print("one"),
                else => std.debug.print("other"),
            }
        }
        fn main(): void { check(1) }
    "#,
    );
    assert!(out.contains("switch (n)"), "got:\n{}", out);
    assert!(out.contains("1 =>"), "got:\n{}", out);
    assert!(out.contains("else =>"), "got:\n{}", out);
}

#[test]
fn test_codegen_enum_decl() {
    let out = compile(
        r#"
        const Direction = enum { North, South, East, West };
        fn main(): void {}
    "#,
    );
    assert!(out.contains("const Direction = enum {"), "got:\n{}", out);
    assert!(out.contains("North,"), "got:\n{}", out);
}

#[test]
fn test_codegen_empty_return() {
    let out = compile(
        r#"
        fn main(): void {
            return
        }
    "#,
    );
    assert!(out.contains("return;"), "got:\n{}", out);
}

#[test]
fn test_codegen_if_expr() {
    let out = compile(
        r#"
        fn choose(x: i32): string {
            return if x > 0 then "positive" else "non-positive"
        }
    "#,
    );
    assert!(
        out.contains("if ((x > 0)) \"positive\" else \"non-positive\""),
        "got:\n{}",
        out
    );
}

#[test]
fn test_codegen_if_expr_uses_std() {
    // std used inside if branches must trigger std import
    let out = compile(
        r#"
        const std = import("std");
        fn greet(flag: bool): void {
            std.debug.print(if flag then "yes" else "no")
        }
    "#,
    );
    assert!(
        out.contains("const std = @import(\"std\")"),
        "expected std import, got:\n{}",
        out
    );
}

#[test]
fn test_codegen_if_expr_alloc_propagation() {
    // allocator-requiring call inside if branch must propagate allocator to caller
    let out = compile(
        r#"
        const std = import("std");
        fn load(flag: bool): !string {
            return if flag then std.fs.readTextFile("a.txt") else std.fs.readTextFile("b.txt")
        }
    "#,
    );
    assert!(
        out.contains("__zyre_allocator"),
        "expected allocator param, got:\n{}",
        out
    );
}

#[test]
fn test_codegen_if_stmt_else() {
    let out = compile(
        r#"
        fn main(): void {
            if true {
                return
            } else {
                return
            }
        }
    "#,
    );
    assert!(out.contains("} else {"), "got:\n{}", out);
}

#[test]
fn test_codegen_if_stmt_else_std() {
    // std used only in else branch must still trigger std import
    let out = compile(
        r#"
        const std = import("std");
        fn main(): void {
            if false {
                return
            } else {
                std.debug.print("else")
            }
        }
    "#,
    );
    assert!(
        out.contains("const std = @import(\"std\")"),
        "got:\n{}",
        out
    );
    assert!(out.contains("} else {"), "got:\n{}", out);
}

#[test]
fn test_codegen_if_expr_nested() {
    let out = compile(
        r#"
        fn classify(x: i32): string {
            return if x > 0 then "pos" else if x == 0 then "zero" else "neg"
        }
    "#,
    );
    assert!(out.contains("if ((x > 0))"), "got:\n{}", out);
    assert!(out.contains("if ((x == 0))"), "got:\n{}", out);
}

#[test]
fn test_codegen_if_expr_as_arg() {
    // if expression passed directly as function argument
    let out = compile(
        r#"
        const std = import("std");
        fn greet(flag: bool): void {
            std.debug.print(if flag then "yes" else "no")
        }
    "#,
    );
    assert!(
        out.contains("__zyre_std_debug.print(if (flag)"),
        "got:\n{}",
        out
    );
}

#[test]
fn test_codegen_if_stmt_no_else() {
    // if without else must not emit "} else {"
    let out = compile(
        r#"
        export fn check(x: i32): void {
            if x > 0 {
                return
            }
        }
    "#,
    );
    assert!(out.contains("if ((x > 0))"), "got:\n{}", out);
    assert!(
        !out.contains("} else {"),
        "unexpected else branch, got:\n{}",
        out
    );
}

#[test]
fn test_codegen_export_const_hoisted() {
    let out = compile(
        r#"
        const a = 2;
        const b = 3;
        export const c = a * b;
    "#,
    );
    assert!(out.contains("pub const c ="), "got:\n{}", out);
    assert!(out.contains("const a ="), "got:\n{}", out);
    assert!(out.contains("const b ="), "got:\n{}", out);
    let main_start = out.find("pub fn main").unwrap_or(out.len());
    assert!(!out[main_start..].contains("const c ="), "got:\n{}", out);
}

#[test]
fn test_codegen_modulo() {
    let out = compile(
        r#"
        export fn rem(a: i32, b: i32): i32 {
            return a % b;
        }
    "#,
    );
    assert!(out.contains("@rem(a, b)"), "got:\n{}", out);
}
