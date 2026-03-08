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
        out.contains("__zyre_print("),
        "expected __zyre_print call, got:\n{}",
        out
    );
    assert!(
        out.contains("fn __zyre_print("),
        "expected __zyre_print definition, got:\n{}",
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
        s.print("hello");
    "#,
    );
    assert!(out.contains("std.debug.print"), "got:\n{}", out);
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
