pub mod build;
pub mod check;
pub mod clean;
pub mod run;

use std::path::Path;

const ZYRE_CACHE_DIR: &str = "zyre-cache";
const ZYRE_OUT_DIR: &str = "zyre-out";

pub fn read_file(path: &str) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|e| panic!("Failed to read '{}': {}", path, e))
}

pub fn full_path(p: &std::path::Path) -> String {
    let s = p.canonicalize().unwrap_or_else(|_| p.to_path_buf());
    let s = s.to_string_lossy();
    s.strip_prefix(r"\\?\").unwrap_or(&s).to_string()
}

pub fn format_error(message: &str, span: (usize, usize), source: &str, path: &str) -> String {
    let (line, col, source_line, caret) = locate(span, source);
    format!(
        "{}:{}:{}: {}\n    {}\n    {}",
        path,
        line,
        col,
        crate::colors::error(message),
        source_line,
        caret
    )
}

pub fn locate(span: (usize, usize), source: &str) -> (usize, usize, &str, String) {
    let before = &source[..span.0.min(source.len())];
    let line = before.bytes().filter(|&b| b == b'\n').count() + 1;
    let line_start = before.rfind('\n').map(|p| p + 1).unwrap_or(0);
    let col = span.0 - line_start + 1;

    let line_end = source[span.0..]
        .find('\n')
        .map(|p| span.0 + p)
        .unwrap_or(source.len());
    let source_line = &source[line_start..line_end];

    let len = (span.1.saturating_sub(span.0)).max(1);
    let caret = " ".repeat(col - 1) + "^" + &"~".repeat(len - 1);

    (line, col, source_line, caret)
}

pub fn abort(n: usize) {
    if n > 1 {
        eprintln!(
            "{}",
            crate::colors::error(&format!("aborting due to {} previous errors", n))
        );
    }
    std::process::exit(1);
}

pub fn collect_zy_imports(
    program: &crate::parser::Program,
    source_dir: &Path,
) -> Vec<(String, String)> {
    let mut result = Vec::new();
    for item in program {
        if let crate::parser::TopLevel::ConstDecl { value, .. } = item {
            if let crate::parser::ExprKind::Import(path) = &value.kind {
                if path.ends_with(".zy") {
                    let rel = path.trim_start_matches("./");
                    let source = source_dir.join(rel);
                    let cache = format!("{}/{}", ZYRE_CACHE_DIR, rel.replace(".zy", ".zig"));
                    result.push((source.to_string_lossy().into_owned(), cache));
                }
            }
        }
    }
    result
}

const ZYRE_RUNTIME: &str = include_str!("../codegen/zig/zyre_runtime.zig");
const ZYRE_STD_DEBUG: &str = include_str!("../codegen/zig/zyre_std_debug.zig");
const ZYRE_STD_FS: &str = include_str!("../codegen/zig/zyre_std_fs.zig");
const ZYRE_TS_STD_DEBUG: &str = include_str!("../codegen/ts/zyre_std_debug.ts");
const ZYRE_TS_STD_FS: &str = include_str!("../codegen/ts/zyre_std_fs.ts");

/// Generates a .zig file in the cache from a pre-parsed AST. Returns (stem, zig_path).
pub fn emit_zig(input_path: &str, ast: &crate::parser::Program) -> (String, String) {
    std::fs::create_dir_all(ZYRE_CACHE_DIR).unwrap();
    for (name, content) in [
        ("zyre_runtime.zig", ZYRE_RUNTIME),
        ("zyre_std_debug.zig", ZYRE_STD_DEBUG),
        ("zyre_std_fs.zig", ZYRE_STD_FS),
    ] {
        std::fs::write(format!("{}/{}", ZYRE_CACHE_DIR, name), content)
            .unwrap_or_else(|e| panic!("Failed to write {}: {}", name, e));
    }

    let mut visited = std::collections::HashSet::new();
    let source_dir = Path::new(input_path).parent().unwrap_or(Path::new("."));
    for (module_src, module_cache) in collect_zy_imports(ast, source_dir) {
        compile_module_inner(&module_src, &module_cache, &mut visited);
    }

    let zig_code = crate::codegen::generate(ast);
    let stem = Path::new(input_path)
        .file_stem()
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    let zig_path = format!("{}/{}.zig", ZYRE_CACHE_DIR, stem);
    std::fs::write(&zig_path, &zig_code)
        .unwrap_or_else(|e| panic!("Failed to write Zig file: {}", e));

    (stem, zig_path)
}

/// Generates a .ts file from a pre-parsed AST. Returns (stem, ts_path).
pub fn emit_ts(input_path: &str, ast: &crate::parser::Program) -> (String, String) {
    std::fs::create_dir_all(ZYRE_OUT_DIR).unwrap();
    for (name, content) in [
        ("zyre_std_debug.ts", ZYRE_TS_STD_DEBUG),
        ("zyre_std_fs.ts", ZYRE_TS_STD_FS),
    ] {
        std::fs::write(format!("{}/{}", ZYRE_OUT_DIR, name), content)
            .unwrap_or_else(|e| panic!("Failed to write {}: {}", name, e));
    }
    use crate::codegen::Backend;
    let mut visited = std::collections::HashSet::new();
    let source_dir = Path::new(input_path).parent().unwrap_or(Path::new("."));
    for item in ast {
        if let crate::parser::TopLevel::ConstDecl { value, .. } = item {
            if let crate::parser::ExprKind::Import(path) = &value.kind {
                if path.ends_with(".zy") {
                    let rel = path.trim_start_matches("./");
                    let source = source_dir.join(rel);
                    let out = Path::new(ZYRE_OUT_DIR).join(rel.replace(".zy", ".ts"));
                    compile_ts_module_inner(
                        &source.to_string_lossy(),
                        &out.to_string_lossy(),
                        &mut visited,
                    );
                }
            }
        }
    }
    let ts_code = crate::codegen::ts::TsBackend::new().generate(ast);
    let stem = Path::new(input_path)
        .file_stem()
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    let ts_path = format!("{}/{}.ts", ZYRE_OUT_DIR, stem);
    std::fs::write(&ts_path, &ts_code).unwrap_or_else(|e| panic!("Failed to write TS file: {}", e));
    (stem, ts_path)
}

fn compile_module_inner(
    source_path: &str,
    cache_path: &str,
    visited: &mut std::collections::HashSet<String>,
) {
    if !visited.insert(source_path.to_string()) {
        return;
    }
    let source = read_file(source_path);
    let tokens = crate::lexer::tokenize(&source);
    let (ast, _, _) = crate::parser::parse(tokens);
    let zig_code = crate::codegen::generate(&ast);

    let cache_dir = Path::new(cache_path).parent().unwrap();
    std::fs::create_dir_all(cache_dir).unwrap();
    std::fs::write(cache_path, &zig_code)
        .unwrap_or_else(|e| panic!("Failed to write '{}': {}", cache_path, e));

    let module_dir = Path::new(source_path).parent().unwrap_or(Path::new("."));
    for (sub_src, sub_cache) in collect_zy_imports(&ast, module_dir) {
        compile_module_inner(&sub_src, &sub_cache, visited);
    }
}

fn compile_ts_module_inner(
    source_path: &str,
    out_path: &str,
    visited: &mut std::collections::HashSet<String>,
) {
    if !visited.insert(source_path.to_string()) {
        return;
    }
    let source = read_file(source_path);
    let tokens = crate::lexer::tokenize(&source);
    let (ast, _, _) = crate::parser::parse(tokens);
    use crate::codegen::Backend;
    let ts_code = crate::codegen::ts::TsBackend::new().generate(&ast);

    let out_dir = Path::new(out_path).parent().unwrap();
    std::fs::create_dir_all(out_dir).unwrap();
    std::fs::write(out_path, &ts_code)
        .unwrap_or_else(|e| panic!("Failed to write '{}': {}", out_path, e));

    let module_dir = Path::new(source_path).parent().unwrap_or(Path::new("."));
    let out_dir = Path::new(out_path)
        .parent()
        .unwrap_or(Path::new(ZYRE_OUT_DIR));
    for item in &ast {
        if let crate::parser::TopLevel::ConstDecl { value, .. } = item {
            if let crate::parser::ExprKind::Import(path) = &value.kind {
                if path.ends_with(".zy") {
                    let rel = path.trim_start_matches("./");
                    let sub_src = module_dir.join(rel);
                    let sub_out = out_dir.join(rel.replace(".zy", ".ts"));
                    compile_ts_module_inner(
                        &sub_src.to_string_lossy(),
                        &sub_out.to_string_lossy(),
                        visited,
                    );
                }
            }
        }
    }
}
