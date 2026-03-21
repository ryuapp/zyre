use super::{emit_ts, emit_zig};
use std::process::Command;

pub fn run_zig(input_path: &str, ast: &crate::parser::Program, opt_level: Option<&str>) {
    let (_, zig_path) = emit_zig(input_path, ast);

    let mut zig_args = vec!["run", &zig_path];
    if let Some(level) = opt_level {
        zig_args.extend_from_slice(&["-O", level]);
    }
    let status = Command::new("zig")
        .args(&zig_args)
        .env("ZIG_LOCAL_CACHE_DIR", "zyre-cache/zig/local")
        .env("ZIG_GLOBAL_CACHE_DIR", "zyre-cache/zig/global")
        .status()
        .unwrap_or_else(|e| panic!("Failed to run zig run: {}", e));
    std::process::exit(status.code().unwrap_or(0));
}

pub fn run_ts(input_path: &str, ast: &crate::parser::Program) {
    let (_, ts_path) = emit_ts(input_path, ast);
    let status = Command::new("deno")
        .args(["run", "--allow-read", "--allow-env", &ts_path])
        .status()
        .unwrap_or_else(|e| panic!("Failed to run deno: {}", e));
    std::process::exit(status.code().unwrap_or(0));
}
