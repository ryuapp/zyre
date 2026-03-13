const RESET: &str = "\x1b[0m";
const BOLD_RED: &str = "\x1b[1;31m";
const RED: &str = "\x1b[31m";
const GREEN: &str = "\x1b[32m";
const RED_BG: &str = "\x1b[97;41m"; // bright white on red bg
const GREEN_BG: &str = "\x1b[97;42m"; // bright white on green bg

fn wrap(color: &str, s: &str) -> String {
    format!("{}{}{}", color, s, RESET)
}

fn wrap_diff(fg: &str, bg: &str, prefix: &str, mid: &str, suffix: &str) -> String {
    format!(
        "{}{}{}{}{}{}{}{}",
        fg, prefix, bg, mid, RESET, fg, suffix, RESET
    )
}

pub fn error(s: &str) -> String {
    format!("{}error{}: {}", BOLD_RED, RESET, s)
}

pub fn red(s: &str) -> String {
    wrap(RED, s)
}

pub fn green(s: &str) -> String {
    wrap(GREEN, s)
}

/// Red line with background highlight on the changed portion.
pub fn red_diff(prefix: &str, mid: &str, suffix: &str) -> String {
    wrap_diff(RED, RED_BG, prefix, mid, suffix)
}

/// Green line with background highlight on the changed portion.
pub fn green_diff(prefix: &str, mid: &str, suffix: &str) -> String {
    wrap_diff(GREEN, GREEN_BG, prefix, mid, suffix)
}
