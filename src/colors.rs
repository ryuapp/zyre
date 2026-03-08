const RESET: &str = "\x1b[0m";
const BOLD_RED: &str = "\x1b[1;31m";

pub fn error(s: &str) -> String {
    format!("{}error{}: {}", BOLD_RED, RESET, s)
}
