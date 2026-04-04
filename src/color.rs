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

#[cfg(test)]
mod tests {
    use super::*;

    // In test environments (no terminal), use_color() returns false.
    // All color functions fall through to the plain `s.to_string()` branch.

    #[test]
    fn test_green_contains_input() {
        let result = green("hello");
        assert!(result.contains("hello"));
    }

    #[test]
    fn test_yellow_contains_input() {
        let result = yellow("world");
        assert!(result.contains("world"));
    }

    #[test]
    fn test_cyan_contains_input() {
        let result = cyan("foo");
        assert!(result.contains("foo"));
    }

    #[test]
    fn test_red_contains_input() {
        let result = red("error_msg");
        assert!(result.contains("error_msg"));
    }

    #[test]
    fn test_bold_contains_input() {
        let result = bold("important");
        assert!(result.contains("important"));
    }

    #[test]
    fn test_dim_contains_input() {
        let result = dim("quiet");
        assert!(result.contains("quiet"));
    }

    #[test]
    fn test_init_no_color_silently_succeeds() {
        // OnceLock may already be set by a prior test; init must not panic
        init(true);
        init(false);
        // Functions still work regardless of lock state
        assert!(!green("ok").is_empty());
    }

    #[test]
    fn test_color_functions_with_numeric_display() {
        let result = green(42);
        assert!(result.contains("42"));
    }
}
