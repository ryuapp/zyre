/// Snapshot tests for generated Zig code.
/// Input fixtures live in src/tests/fixtures/*.zy
/// Run `cargo insta review` to approve new/changed snapshots.

#[test]
fn test_codegen_snapshots() {
    insta::glob!("fixtures/*.zy", |path| {
        let src = std::fs::read_to_string(path).unwrap();
        let output = super::compile(&src);
        insta::assert_snapshot!(output);
    });
}

/// Snapshot tests for generated TypeScript code.
#[test]
fn test_ts_snapshots() {
    insta::glob!("fixtures/*.zy", |path| {
        let src = std::fs::read_to_string(path).unwrap();
        let output = super::compile_ts(&src);
        insta::assert_snapshot!(output);
    });
}
