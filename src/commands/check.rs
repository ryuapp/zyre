use super::abort;
use std::path::Path;

pub fn run(input_path: Option<&str>, fix: bool) {
    match input_path {
        None => {
            let files = collect_zy_files(Path::new("."));
            let total = files.len();
            let mut type_error_count = 0usize;
            let mut not_formatted = 0usize;
            for p in &files {
                let (type_errors, fmt_diff, _changed) = check_file(p, fix);
                if !type_errors.is_empty() {
                    eprintln!("{}", super::full_path(p));
                    for msg in &type_errors {
                        eprintln!("{}", msg);
                    }
                    type_error_count += type_errors.len();
                }
                if let Some(diff) = &fmt_diff {
                    eprintln!("from {}:", super::full_path(p));
                    eprintln!("{}", diff);
                    not_formatted += 1;
                }
            }
            print_summary(total, type_error_count, not_formatted);
        }
        Some(path) => {
            let source = super::read_file(path);
            let (_, type_errors, fmt_diff, _changed) = check_and_report(&source, path, fix);
            if !type_errors.is_empty() {
                eprintln!("{}", super::full_path(Path::new(path)));
                for msg in &type_errors {
                    eprintln!("{}", msg);
                }
            }
            let not_formatted = if let Some(diff) = &fmt_diff {
                eprintln!("from {}:", super::full_path(Path::new(path)));
                eprintln!("{}", diff);
                1
            } else {
                0
            };
            print_summary(1, type_errors.len(), not_formatted);
            if !type_errors.is_empty() {
                abort(type_errors.len());
            }
        }
    }
}

fn plural(n: usize) -> &'static str {
    if n == 1 { "" } else { "s" }
}

fn print_summary(total: usize, type_errors: usize, not_formatted: usize) {
    if not_formatted > 0 {
        eprintln!(
            "{}",
            crate::colors::error(&format!(
                "Found {} not formatted file{} in {} file{}",
                not_formatted,
                plural(not_formatted),
                total,
                plural(total),
            ))
        );
        if type_errors == 0 {
            std::process::exit(1);
        }
    } else if type_errors == 0 {
        eprintln!("Checked {} file{}", total, plural(total));
    }
    if type_errors > 0 {
        std::process::exit(1);
    }
}

/// Parses and type-checks source, aborting on errors. Returns the AST.
pub fn check_source(source: &str, path: &str) -> crate::parser::Program {
    let (ast, type_errors, _, _) = check_and_report(source, path, false);
    if !type_errors.is_empty() {
        for msg in &type_errors {
            eprintln!("{}", msg);
        }
        abort(type_errors.len());
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

fn check_file(path: &Path, fix: bool) -> (Vec<String>, Option<String>, bool) {
    let path_str = path.to_string_lossy();
    let source = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            return (vec![format!("{}: {}", path_str, e)], None, false);
        }
    };
    let (_, type_errors, fmt_diff, changed) = check_and_report(&source, &path_str, fix);
    (type_errors, fmt_diff, changed)
}

fn diff_highlight(a: &str, b: &str) -> (String, String) {
    let ac: Vec<char> = a.chars().collect();
    let bc: Vec<char> = b.chars().collect();
    let prefix = ac.iter().zip(bc.iter()).take_while(|(x, y)| x == y).count();
    let suffix = ac[prefix..]
        .iter()
        .rev()
        .zip(bc[prefix..].iter().rev())
        .take_while(|(x, y)| x == y)
        .count();
    let a_end = ac.len() - suffix;
    let b_end = bc.len() - suffix;
    let ap: String = ac[..prefix].iter().collect();
    let am: String = ac[prefix..a_end].iter().collect();
    let as_: String = ac[a_end..].iter().collect();
    let bp: String = bc[..prefix].iter().collect();
    let bm: String = bc[prefix..b_end].iter().collect();
    let bs: String = bc[b_end..].iter().collect();
    (
        crate::colors::red_diff(&ap, &am, &as_),
        crate::colors::green_diff(&bp, &bm, &bs),
    )
}

fn fmt_diff(source: &str, formatted: &str) -> String {
    let src_lines: Vec<&str> = source.lines().collect();
    let fmt_lines: Vec<&str> = formatted.lines().collect();
    let mut out = String::new();
    let max_len = src_lines.len().max(fmt_lines.len());
    let mut shown = 0;
    for i in 0..max_len {
        let a = src_lines.get(i).copied().unwrap_or("");
        let b = fmt_lines.get(i).copied().unwrap_or("");
        if a != b {
            let (hl_a, hl_b) = diff_highlight(a, b);
            let ln = i + 1;
            out.push_str(&crate::colors::red(&format!("{} | -", ln)));
            out.push_str(&hl_a);
            out.push('\n');
            out.push_str(&crate::colors::green(&format!("{} | +", ln)));
            out.push_str(&hl_b);
            out.push('\n');
            shown += 1;
            if shown >= 8 {
                out.push_str("...\n");
                break;
            }
        }
    }
    if out.is_empty() && source != formatted {
        // Only difference is trailing newline — show as an added empty line
        let ln = source.lines().count() + 1;
        out.push_str(&crate::colors::green(&format!("{} | +", ln)));
    } else {
        let trimmed_len = out.trim_end().len();
        out.truncate(trimmed_len);
    }
    out
}

fn check_and_report(
    source: &str,
    path: &str,
    fix: bool,
) -> (crate::parser::Program, Vec<String>, Option<String>, bool) {
    let tokens = crate::lexer::tokenize(source);
    let (ast, parse_errors, blanks) = crate::parser::parse(tokens);
    let mut type_errors: Vec<String> = Vec::new();

    for e in &parse_errors {
        type_errors.push(super::format_error(&e.message, e.span, source, path));
    }
    let source_dir = Path::new(path).parent().unwrap_or(Path::new("."));
    let zy_errors = crate::typechecker::check_with_diagnostics(&ast, source_dir);
    for e in &zy_errors {
        if let Some(span) = e.span {
            type_errors.push(super::format_error(&e.message, span, source, path));
        } else {
            type_errors.push(crate::colors::error(&e.message).to_string());
        }
    }

    let mut fmt_diff_out: Option<String> = None;
    let mut changed = false;
    if parse_errors.is_empty() {
        let formatted = crate::fmt::format_program(&ast, &blanks);
        if formatted != source {
            if fix {
                std::fs::write(path, &formatted)
                    .unwrap_or_else(|e| panic!("Failed to write '{}': {}", path, e));
                changed = true;
            } else {
                fmt_diff_out = Some(fmt_diff(source, &formatted));
            }
        }
    }

    (ast, type_errors, fmt_diff_out, changed)
}
