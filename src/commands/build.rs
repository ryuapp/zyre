use super::emit_zig;
use std::process::Command;

pub fn run(input_path: &str, ast: &crate::parser::Program, opt_level: Option<&str>) {
    let (stem, zig_path) = emit_zig(input_path, ast);

    let exe_path = if cfg!(windows) {
        format!("zyre-out/{}.exe", stem)
    } else {
        format!("zyre-out/{}", stem)
    };
    let emit_bin = format!("-femit-bin={}", exe_path);
    let mut zig_args = vec!["build-exe", &zig_path, "--name", &stem, &emit_bin];
    if let Some(level) = opt_level {
        zig_args.extend_from_slice(&["-O", level]);
    }
    let status = Command::new("zig")
        .args(&zig_args)
        .status()
        .unwrap_or_else(|e| panic!("Failed to run zig build-exe: {}", e));
    if !status.success() {
        eprintln!("Compilation failed");
        std::process::exit(1);
    }
}
