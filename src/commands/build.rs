use super::emit_ts;

pub fn build_ts(input_path: &str, ast: &crate::parser::Program) {
    let (_, ts_path) = emit_ts(input_path, ast);
    eprintln!("{}", ts_path);
}
