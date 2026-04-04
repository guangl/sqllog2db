use std::io::IsTerminal;
use std::sync::OnceLock;

static USE_COLOR: OnceLock<bool> = OnceLock::new();

/// 在解析完 CLI flags 后调用一次，明确设置颜色开关。
/// 若已初始化（例如测试环境）则忽略。
pub fn init(no_color: bool) {
    let _ = USE_COLOR.set(
        !no_color
            && std::env::var("NO_COLOR").is_err()
            && (std::io::stdout().is_terminal() || std::io::stderr().is_terminal()),
    );
}

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
