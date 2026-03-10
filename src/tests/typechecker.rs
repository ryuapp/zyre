use super::{err, ok};

#[test]
fn test_type_mismatch() {
    err(
        r#"
        fn main(): void {
            const x: i32 = "hello";
        }
    "#,
        "Type mismatch",
    );
}

#[test]
fn test_undefined_variable() {
    err(
        r#"
        fn main(): void {
            const x = y;
        }
    "#,
        "Undefined variable",
    );
}

#[test]
fn test_return_type_mismatch() {
    err(
        r#"
        fn foo(): i32 {
            return "wrong";
        }
        fn main(): void {}
    "#,
        "Return type mismatch",
    );
}

#[test]
fn test_if_condition_not_bool() {
    err(
        r#"
        fn main(): void {
            if 42 {
                const x = 1;
            }
        }
    "#,
        "if condition must be bool",
    );
}

#[test]
fn test_std_print_arg_count() {
    err(
        r#"
        const std = import("std");
        fn main(): void {
            std.debug.print("a", "b");
        }
    "#,
        "std.debug.print expects 1 argument",
    );
}

// --- Function argument checks ---

#[test]
fn test_fn_arg_count_mismatch() {
    err(
        r#"
        fn add(a: i32, b: i32): i32 { return a + b; }
        fn main(): void {
            const x = add(1);
        }
    "#,
        "Expected 2 argument(s), got 1",
    );
}

#[test]
fn test_fn_arg_type_mismatch() {
    err(
        r#"
        fn add(a: i32, b: i32): i32 { return a + b; }
        fn main(): void {
            const x = add("hello", true);
        }
    "#,
        "Argument 1 type mismatch",
    );
}

#[test]
fn test_fn_arg_ok() {
    ok(r#"
        fn add(a: i32, b: i32): i32 { return a + b; }
        fn main(): void {
            const _x = add(1, 2);
        }
    "#);
}

// --- Struct field access ---

#[test]
fn test_struct_field_access() {
    ok(r#"
        const Point = struct { x: i32, y: i32 };
        fn getX(p: Point): i32 {
            return p.x;
        }
    "#);
}

#[test]
fn test_struct_field_unknown() {
    err(
        r#"
        const Point = struct { x: i32, y: i32 };
        fn getZ(p: Point): i32 {
            return p.z;
        }
    "#,
        "Struct 'Point' has no field 'z'",
    );
}

#[test]
fn test_toplevel_unused_alias() {
    err(
        r#"
        const std = import("std");
        const a = std;
        std.debug.print("hello");
    "#,
        "Unused top-level alias: 'a'",
    );
}

// --- Enum ---

#[test]
fn test_enum_ok() {
    ok(r#"
        const Direction = enum { North, South, East, West };
        fn go(d: Direction): void {}
        fn main(): void {}
    "#);
}

// --- Switch ---

#[test]
fn test_switch_int_ok() {
    ok(r#"
        const std = import("std");
        fn check(n: i32): void {
            switch n {
                1 => std.debug.print("one"),
                else => std.debug.print("other"),
            }
        }
        fn main(): void {}
    "#);
}

#[test]
fn test_switch_enum_ok() {
    ok(r#"
        const std = import("std");
        const D = enum { A, B };
        fn go(d: D): void {
            switch d {
                A => std.debug.print("a"),
                B => std.debug.print("b"),
            }
        }
        fn main(): void {}
    "#);
}

// --- Optional ---

#[test]
fn test_optional_param_ok() {
    ok(r#"
        fn maybe(x: ?i32): void {}
        fn main(): void {}
    "#);
}

// --- Empty return ---

#[test]
fn test_empty_return_ok() {
    ok(r#"
        fn main(): void {
            return
        }
    "#);
}

// --- If statement with else ---

#[test]
fn test_if_stmt_else_ok() {
    ok(r#"
        fn main(): void {
            if true {
                return
            } else {
                return
            }
        }
    "#);
}

#[test]
fn test_if_stmt_else_condition_not_bool() {
    err(
        r#"
        fn main(): void {
            if 42 {
                return
            } else {
                return
            }
        }
    "#,
        "if condition must be bool",
    );
}

// --- If expression ---

#[test]
fn test_if_expr_ok() {
    ok(r#"
        fn choose(x: i32): string {
            return if x > 0 then "positive" else "non-positive"
        }
        fn main(): void {}
    "#);
}

#[test]
fn test_if_expr_branch_type_mismatch() {
    err(
        r#"
        fn choose(x: i32): string {
            return if x > 0 then "positive" else 42
        }
        fn main(): void {}
    "#,
        "if branches have different types",
    );
}
