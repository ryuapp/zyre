use super::emit_zig;
use std::process::Command;

pub fn run(input_path: &str, ast: &crate::parser::Program, opt_level: Option<&str>) {
    let (_, zig_path) = emit_zig(input_path, ast);

    let mut zig_args = vec!["run", &zig_path];
    if let Some(level) = opt_level {
        zig_args.extend_from_slice(&["-O", level]);
    }
    let status = Command::new("zig")
        .args(&zig_args)
        .status()
        .unwrap_or_else(|e| panic!("Failed to run zig run: {}", e));
    std::process::exit(status.code().unwrap_or(0));
}
