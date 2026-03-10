use std::process::Command;

fn run_example(path: &std::path::Path) {
    let bin = env!("CARGO_BIN_EXE_zyre");
    let out = Command::new(bin)
        .args(["run", &path.to_string_lossy()])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .unwrap_or_else(|e| panic!("failed to spawn zyre for {}: {}", path.display(), e));
    assert!(
        out.status.success(),
        "{} exited with non-zero status",
        path.display()
    );
}

#[test]
fn test_examples() {
    let examples = std::fs::read_dir("examples")
        .expect("examples/ directory not found")
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map(|x| x == "zy").unwrap_or(false))
        .map(|e| e.path());

    for path in examples {
        run_example(&path);
    }
}
