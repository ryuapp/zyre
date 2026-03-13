fn check(src: &str, expected: &str) {
    let out = fmt(src);
    assert_eq!(out.trim(), expected.trim(), "fmt output mismatch");
    idempotent(src);
}

fn fmt(src: &str) -> String {
    let tokens = crate::lexer::tokenize(src);
    let (ast, _, blanks) = crate::parser::parse(tokens);
    crate::fmt::format_program(&ast, &blanks)
}

fn idempotent(src: &str) {
    let once = fmt(src);
    let twice = fmt(&once);
    assert_eq!(once, twice, "fmt is not idempotent:\n{}", once);
}

#[test]
fn test_fmt_const() {
    check("const x = 42", "const x = 42");
}

#[test]
fn test_fmt_const_typed() {
    check(
        "fn main(): void { const x: i32 = 42 }",
        "fn main(): void {\n    const x: i32 = 42\n}",
    );
}

#[test]
fn test_fmt_let() {
    check(
        "fn main(): void { let x = 0 }",
        "fn main(): void {\n    let x = 0\n}",
    );
}

#[test]
fn test_fmt_let_typed() {
    check(
        "fn main(): void { let x: i32 = 0 }",
        "fn main(): void {\n    let x: i32 = 0\n}",
    );
}

#[test]
fn test_fmt_string_literal() {
    check(r#"const s = "hello""#, r#"const s = "hello""#);
}

#[test]
fn test_fmt_float() {
    check("const f = 3.14", "const f = 3.14");
}

#[test]
fn test_fmt_bool() {
    check("const b = true", "const b = true");
}

#[test]
fn test_fmt_return_void() {
    check(
        "fn main(): void { return }",
        "fn main(): void {\n    return\n}",
    );
}

#[test]
fn test_fmt_break_continue() {
    let src = "fn main(): void { while true { break } }";
    let out = fmt(src);
    assert!(out.contains("break"), "got: {}", out);
    idempotent(&out);
}

#[test]
fn test_fmt_array_literal() {
    check(
        "fn main(): void { const a: i32[3] = [1, 2, 3] }",
        "fn main(): void {\n    const a: i32[3] = [1, 2, 3]\n}",
    );
}

#[test]
fn test_fmt_array_index() {
    check(
        "fn main(): void { const x = a[0] }",
        "fn main(): void {\n    const x = a[0]\n}",
    );
}

#[test]
fn test_fmt_optional_type() {
    check(
        "fn f(x: ?i32): ?string { return null }",
        "fn f(x: ?i32): ?string {\n    return null\n}",
    );
}

#[test]
fn test_fmt_error_union_type() {
    check(
        "fn f(): !string { return \"ok\" }",
        "fn f(): !string {\n    return \"ok\"\n}",
    );
}

#[test]
fn test_fmt_member_access() {
    check("const x = obj.field", "const x = obj.field");
}

#[test]
fn test_fmt_call() {
    check(
        "fn main(): void { foo(1, 2) }",
        "fn main(): void {\n    foo(1, 2)\n}",
    );
}

#[test]
fn test_fmt_propagate() {
    check(
        "fn main(): void { const x = foo()? }",
        "fn main(): void {\n    const x = foo()?\n}",
    );
}

#[test]
fn test_fmt_unary_neg() {
    check("const x = -1", "const x = -1");
}

#[test]
fn test_fmt_unary_not() {
    check("const x = !true", "const x = !true");
}

#[test]
fn test_fmt_logical_operators() {
    check(
        "fn main(): void { const x = a and b }",
        "fn main(): void {\n    const x = a and b\n}",
    );
    check(
        "fn main(): void { const x = a or b }",
        "fn main(): void {\n    const x = a or b\n}",
    );
}

#[test]
fn test_fmt_export_fn() {
    let out = fmt("export fn add(a: i32, b: i32): i32 { return a + b }");
    assert!(out.contains("export fn add"), "got: {}", out);
    idempotent(&out);
}

#[test]
fn test_fmt_export_const() {
    check("export const PI = 3.14", "export const PI = 3.14");
}

#[test]
fn test_fmt_import() {
    check(
        r#"const std = import("std")"#,
        r#"const std = import("std")"#,
    );
}

#[test]
fn test_fmt_switch_expr() {
    let src = r#"const s = switch x {
    1 => "one",
    else => "other",
}"#;
    idempotent(src);
}

#[test]
fn test_fmt_nested_if_expr() {
    let src = r#"fn sign(x: i32): string {
    return if x > 0 then "positive" else if x < 0 then "negative" else "zero"
}"#;
    idempotent(src);
}

#[test]
fn test_fmt_catch() {
    let src = r#"fn main(): void {
    const data = readFile("foo")?
}"#;
    let out = fmt(src);
    assert!(
        out.contains("const data = readFile(\"foo\")?"),
        "got: {}",
        out
    );
    idempotent(&out);
}

#[test]
fn test_fmt_fn() {
    let src = r#"fn add(a: i32, b: i32): i32 {
    return a + b
}"#;
    idempotent(src);
    assert!(fmt(src).contains("fn add(a: i32, b: i32): i32 {"));
}

#[test]
fn test_fmt_if_stmt() {
    let src = r#"fn main(): void {
    if x > 0 {
        const y = 1
    }
}"#;
    idempotent(src);
}

#[test]
fn test_fmt_if_else_stmt() {
    let src = r#"fn main(): void {
    if x > 0 {
        const y = 1
    } else {
        const y = 2
    }
}"#;
    idempotent(src);
}

#[test]
fn test_fmt_if_expr() {
    let out = fmt("fn abs(x: i32): i32 { return if x < 0 then -x else x }");
    assert!(out.contains("return if x < 0 then -x else x"));
    idempotent(&out);
}

#[test]
fn test_fmt_while() {
    let src = r#"fn main(): void {
    let i = 0
    while i < 10 {
        i = i + 1
    }
}"#;
    idempotent(src);
}

#[test]
fn test_fmt_binop_precedence() {
    // lower-precedence subexpr should be parenthesized
    let out = fmt("const x = (a + b) * c");
    assert!(out.contains("(a + b) * c"), "got: {}", out);
    idempotent(&out);
}

#[test]
fn test_fmt_blank_lines_between_fns() {
    let src = "fn a(): void {}\n\nfn b(): void {}";
    let out = fmt(src);
    assert!(
        out.contains("\n\n"),
        "expected blank line between fns, got:\n{}",
        out
    );
    idempotent(&out);
}

#[test]
fn test_fmt_struct() {
    let src = r#"const Point = struct {
    x: f64,
    y: f64,
}"#;
    idempotent(src);
}

#[test]
fn test_fmt_enum() {
    let src = r#"const Dir = enum {
    North,
    South,
}"#;
    idempotent(src);
}

#[test]
fn test_fmt_modulo() {
    let out = fmt("fn rem(a: i32, b: i32): i32 { return a % b }");
    assert!(out.contains("a % b"), "got: {}", out);
    idempotent(&out);
}

#[test]
fn test_fmt_fixtures_idempotent() {
    let fixtures = std::fs::read_dir("src/tests/fixtures")
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map(|x| x == "zy").unwrap_or(false));

    for entry in fixtures {
        let src = std::fs::read_to_string(entry.path()).unwrap();
        idempotent(&src);
    }
}
