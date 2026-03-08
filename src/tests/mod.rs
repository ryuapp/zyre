use crate::lexer::tokenize;
use crate::parser::parse;
use crate::typechecker::check;

mod codegen;
mod lexer;
mod snapshots;
mod stdlib;
mod typechecker;

pub(super) fn run(src: &str) -> Vec<String> {
    let tokens = tokenize(src);
    let (ast, _) = parse(tokens);
    check(&ast, std::path::Path::new("."))
}

pub(super) fn ok(src: &str) {
    let errors = run(src);
    assert!(errors.is_empty(), "Expected no errors, got: {:?}", errors);
}

pub(super) fn err(src: &str, expected: &str) {
    let errors = run(src);
    assert!(
        errors.iter().any(|e| e.contains(expected)),
        "Expected error containing '{}', got: {:?}",
        expected,
        errors
    );
}

pub(super) fn compile(src: &str) -> String {
    let tokens = tokenize(src);
    let (ast, _) = parse(tokens);
    crate::codegen::generate(&ast)
}

// --- Happy path tests ---

#[test]
fn test_hello() {
    ok(r#"
        const std = import("std");
        fn main(): void {
            std.debug.print("Hello");
        }
    "#);
}

#[test]
fn test_const_infer() {
    ok(r#"
        fn main(): void {
            const _x = 42;
            const _y = 3.14;
            const _s = "hello";
            const _b = true;
        }
    "#);
}

#[test]
fn test_let() {
    ok(r#"
        fn main(): void {
            let _x: i32 = 0;
            let _y = false;
        }
    "#);
}

#[test]
fn test_arithmetic() {
    ok(r#"
        fn add(a: i32, b: i32): i32 {
            return a + b;
        }
        fn main(): void {
            const _result = add(1, 2);
        }
    "#);
}

#[test]
fn test_if() {
    ok(r#"
        fn main(): void {
            const x = true;
            if x {
                const _y = 1;
            }
        }
    "#);
}

#[test]
fn test_while() {
    ok(r#"
        fn main(): void {
            let i = 0;
            while i == 0 {
                break;
            }
        }
    "#);
}

// --- Arrays ---

#[test]
fn test_array_decl() {
    ok(r#"
        fn main(): void {
            const _arr: i32[3] = [1, 2, 3];
        }
    "#);
}

#[test]
fn test_array_index() {
    ok(r#"
        fn main(): void {
            const arr: i32[3] = [1, 2, 3];
            const _x = arr[0];
        }
    "#);
}

#[test]
fn test_array_element_type_mismatch() {
    err(
        r#"
        fn main(): void {
            const arr: i32[3] = [1, "two", 3];
        }
    "#,
        "Array element type mismatch",
    );
}

#[test]
fn test_array_index_not_i32() {
    err(
        r#"
        fn main(): void {
            const arr: i32[3] = [1, 2, 3];
            const x = arr[true];
        }
    "#,
        "Array index must be i32",
    );
}

// --- import / export ---

#[test]
fn test_import_alias() {
    ok(r#"
        const s = import("std");
        s.print("hello");
    "#);
}

#[test]
fn test_toplevel_stmt() {
    ok(r#"
        const std = import("std");
        const x = 42;
        std.debug.print("hi");
    "#);
}

#[test]
fn test_local_module_import() {
    ok(r#"
        const math = import("./modules/math.zy");
        const result = math.add(1, 2);
    "#);
}

#[test]
fn test_export_fn() {
    ok(r#"
        export fn add(a: i32, b: i32): i32 {
            return a + b;
        }
    "#);
}
