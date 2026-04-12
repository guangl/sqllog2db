/// 将 SQL 字符串转为指纹：字面量替换为 `?`，折叠连续空白。
///
/// 结构相同但参数不同的 SQL 将得到同一指纹，用于 `digest` 命令聚合。
///
/// # Panics
///
/// 内部断言：`sql` 是有效 UTF-8（函数签名已保证），不会在实际中触发。
#[must_use]
pub fn fingerprint(sql: &str) -> String {
    let bytes = sql.as_bytes();
    let len = bytes.len();
    let mut out: Vec<u8> = Vec::with_capacity(sql.len());
    let mut i = 0;

    while i < len {
        match bytes[i] {
            b'\'' => {
                out.push(b'?');
                i += 1;
                // 用 memchr 跳到下一个引号，避免逐字节扫描
                loop {
                    let Some(rel) = memchr::memchr(b'\'', &bytes[i..]) else {
                        i = len;
                        break;
                    };
                    i += rel + 1;
                    if i < len && bytes[i] == b'\'' {
                        i += 1; // '' 转义，继续消费
                    } else {
                        break;
                    }
                }
            }
            b if b.is_ascii_digit() && !prev_is_ident_byte(&out) => {
                out.push(b'?');
                i += 1;
                while i < len && (bytes[i].is_ascii_digit() || bytes[i] == b'.') {
                    i += 1;
                }
            }
            b if b.is_ascii_whitespace() => {
                if !matches!(out.last(), Some(&b' ')) {
                    out.push(b' ');
                }
                i += 1;
                while i < len && bytes[i].is_ascii_whitespace() {
                    i += 1;
                }
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }

    // `out` 的来源只有两类字节：
    //   1. `sql` 原始字节（已是有效 UTF-8）—— 单引号 0x27、ASCII 数字 0x30-0x39、
    //      ASCII 空白均不可能是 UTF-8 多字节序列的后续字节（>= 0x80），
    //      因此多字节字符不会被拆断。
    //   2. 我们插入的 ASCII 字节：b'?' (0x3F) 和 b' ' (0x20)。
    // 故 `from_utf8` 始终成功。
    let out_str = String::from_utf8(out).expect("fingerprint: invalid UTF-8");
    // 折叠后最多只有首尾各一个空格；无需 trim 时直接返回，避免额外分配
    let trimmed = out_str.trim_ascii();
    if trimmed.len() == out_str.len() {
        out_str
    } else {
        trimmed.to_string()
    }
}

/// 上一个输出字节是否是标识符字节（字母/数字/下划线/点）
fn prev_is_ident_byte(out: &[u8]) -> bool {
    out.last()
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
