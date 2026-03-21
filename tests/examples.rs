use std::process::Command;

fn run_example(path: &std::path::Path, args: &[&str]) -> std::process::Output {
    let bin = env!("CARGO_BIN_EXE_zyre");
    Command::new(bin)
        .args(args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .unwrap_or_else(|e| panic!("failed to spawn zyre for {}: {}", path.display(), e))
}

fn run_example_ts(path: &std::path::Path) {
    let path_arg = path.to_string_lossy();
    let out = run_example(path, &["run", "--ts", &path_arg]);
    assert!(
        out.status.success(),
        "{} exited with non-zero status\nstdout:\n{}\nstderr:\n{}",
        path.display(),
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
}

fn run_example_zig(path: &std::path::Path) {
    let path_arg = path.to_string_lossy();
    let out = run_example(path, &["run", &path_arg]);
    let stderr = String::from_utf8_lossy(&out.stderr);
    let infra_error = stderr.contains("manifest_create AccessDenied")
        || stderr.contains("UnableToSpawnSelf")
        || stderr.contains("unable to spawn LLD");
    if infra_error {
        eprintln!(
            "skipping zig example {} due to environment issue:\n{}",
            path.display(),
            stderr
        );
        return;
    }
    assert!(
        out.status.success(),
        "{} exited with non-zero status\nstdout:\n{}\nstderr:\n{}",
        path.display(),
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
}

fn example_paths() -> impl Iterator<Item = std::path::PathBuf> {
    std::fs::read_dir("examples")
        .expect("examples/ directory not found")
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map(|x| x == "zy").unwrap_or(false))
        .map(|e| e.path())
}

mod ts {
    #[test]
    fn test_examples() {
        for path in super::example_paths() {
            super::run_example_ts(&path);
        }
    }
}

mod zig {
    #[test]
    fn test_examples() {
        for path in super::example_paths() {
            super::run_example_zig(&path);
        }
    }
}
