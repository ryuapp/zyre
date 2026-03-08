use super::{abort, render_error};
use std::path::Path;

pub fn run(input_path: Option<&str>) {
    match input_path {
        None => {
            let files = collect_zy_files(Path::new("."));
            let total_errors: usize = files.iter().map(|p| check_file(p)).sum();
            if total_errors > 0 {
                std::process::exit(1);
            }
        }
        Some(path) => {
            check_source(&super::read_file(path), path);
        }
    }
}

/// Parses and type-checks source, aborting on errors. Returns the AST.
pub fn check_source(source: &str, path: &str) -> crate::parser::Program {
    let (ast, error_count) = check_and_report(source, path);
    if error_count > 0 {
        abort(error_count);
    }
    ast
}

fn collect_zy_files(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut result = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                result.extend(collect_zy_files(&path));
            } else if path.extension().and_then(|e| e.to_str()) == Some("zy") {
                result.push(path);
            }
        }
    }
    result.sort();
    result
}

fn check_file(path: &Path) -> usize {
    let path_str = path.to_string_lossy();
    let source = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{}: {}", path_str, e);
            return 1;
        }
    };
    let (_, error_count) = check_and_report(&source, &path_str);
    error_count
}

fn check_and_report(source: &str, path: &str) -> (crate::parser::Program, usize) {
    let tokens = crate::lexer::tokenize(source);
    let (ast, parse_errors) = crate::parser::parse(tokens);
    let mut error_count = parse_errors.len();
    for e in &parse_errors {
        render_error(&e.message, e.span, source, path);
    }
    let source_dir = Path::new(path).parent().unwrap_or(Path::new("."));
    let zy_errors = crate::typechecker::check_with_diagnostics(&ast, source_dir);
    error_count += zy_errors.len();
    for e in &zy_errors {
        if let Some(span) = e.span {
            render_error(&e.message, span, source, path);
        } else {
            eprintln!("{}", crate::colors::error(&e.message));
        }
    }
    (ast, error_count)
}
