use std::io::IsTerminal;
use std::sync::OnceLock;

static USE_COLOR: OnceLock<bool> = OnceLock::new();

fn use_color() -> bool {
    *USE_COLOR.get_or_init(|| {
        std::env::var("NO_COLOR").is_err()
            && (std::io::stdout().is_terminal() || std::io::stderr().is_terminal())
    })
}

pub fn green(s: impl std::fmt::Display) -> String {
    if use_color() {
        format!("\x1b[32m{s}\x1b[0m")
    } else {
        s.to_string()
    }
}

pub fn yellow(s: impl std::fmt::Display) -> String {
    if use_color() {
        format!("\x1b[33m{s}\x1b[0m")
    } else {
        s.to_string()
    }
}

pub fn cyan(s: impl std::fmt::Display) -> String {
    if use_color() {
        format!("\x1b[36m{s}\x1b[0m")
    } else {
        s.to_string()
    }
}

#[allow(dead_code)]
pub fn red(s: impl std::fmt::Display) -> String {
    if use_color() {
        format!("\x1b[31m{s}\x1b[0m")
    } else {
        s.to_string()
    }
}

pub fn bold(s: impl std::fmt::Display) -> String {
    if use_color() {
        format!("\x1b[1m{s}\x1b[0m")
    } else {
        s.to_string()
    }
}

pub fn dim(s: impl std::fmt::Display) -> String {
    if use_color() {
        format!("\x1b[2m{s}\x1b[0m")
    } else {
        s.to_string()
    }
}
