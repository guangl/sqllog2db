use std::iter::Peekable;
use std::str::Chars;

/// 将 SQL 字符串转为指纹：字面量替换为 `?`，折叠连续空白。
///
/// 结构相同但参数不同的 SQL 将得到同一指纹，用于 `digest` 命令聚合。
#[must_use]
pub fn fingerprint(sql: &str) -> String {
    let mut out = String::with_capacity(sql.len());
    let mut chars = sql.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '\'' => consume_string_literal(&mut chars, &mut out),
            _ if ch.is_ascii_digit() && !prev_is_ident_byte(&out) => {
                consume_number_literal(&mut chars, &mut out);
            }
            _ if ch.is_whitespace() => collapse_whitespace(&mut chars, &mut out),
            _ => out.push(ch),
        }
    }
    out.trim().to_string()
}

/// 消费单引号字符串直到匹配的结束引号，输出 `?`
fn consume_string_literal(chars: &mut Peekable<Chars<'_>>, out: &mut String) {
    out.push('?');
    loop {
        match chars.next() {
            None => break,
            Some('\'') => {
                if chars.peek() == Some(&'\'') {
                    chars.next(); // '' 转义，继续
                } else {
                    break;
                }
            }
            Some(_) => {}
        }
    }
}

/// 消费数字字面量（含小数点），输出 `?`
fn consume_number_literal(chars: &mut Peekable<Chars<'_>>, out: &mut String) {
    out.push('?');
    while chars
        .peek()
        .is_some_and(|c| c.is_ascii_digit() || *c == '.')
    {
        chars.next();
    }
}

/// 将连续空白折叠为单个空格
fn collapse_whitespace(chars: &mut Peekable<Chars<'_>>, out: &mut String) {
    if out.as_bytes().last().is_some_and(|&b| b != b' ') {
        out.push(' ');
    }
    while chars.peek().is_some_and(|c| c.is_whitespace()) {
        chars.next();
    }
}

/// 上一个输出字符是否是标识符字符（字母/数字/下划线/点）
fn prev_is_ident_byte(out: &str) -> bool {
    out.as_bytes()
        .last()
        .is_some_and(|&b| b.is_ascii_alphanumeric() || b == b'_' || b == b'.')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_string_literal_replaced() {
        assert_eq!(fingerprint("WHERE name = 'alice'"), "WHERE name = ?");
    }

    #[test]
    fn test_number_literal_replaced() {
        assert_eq!(fingerprint("WHERE id = 123"), "WHERE id = ?");
    }

    #[test]
    fn test_decimal_literal_replaced() {
        assert_eq!(fingerprint("WHERE price > 9.99"), "WHERE price > ?");
    }

    #[test]
    fn test_identifier_with_digit_preserved() {
        assert_eq!(fingerprint("col1 = 5"), "col1 = ?");
    }

    #[test]
    fn test_whitespace_collapsed() {
        assert_eq!(fingerprint("SELECT  *  FROM  t"), "SELECT * FROM t");
    }

    #[test]
    fn test_escaped_quote_in_string() {
        assert_eq!(fingerprint("WHERE name = 'it''s'"), "WHERE name = ?");
    }

    #[test]
    fn test_multiple_literals() {
        let sql = "INSERT INTO t (a, b) VALUES ('x', 42)";
        assert_eq!(fingerprint(sql), "INSERT INTO t (a, b) VALUES (?, ?)");
    }

    #[test]
    fn test_empty_string() {
        assert_eq!(fingerprint(""), "");
    }

    #[test]
    fn test_same_fingerprint_for_different_values() {
        let sql1 = "SELECT * FROM t WHERE id = 1";
        let sql2 = "SELECT * FROM t WHERE id = 999";
        assert_eq!(fingerprint(sql1), fingerprint(sql2));
    }
}
