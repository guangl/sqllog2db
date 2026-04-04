/// 将 SQL 中的参数字面量替换为占位符 `?`，用于 SQL 模式归一化。
///
/// 替换规则：
/// - 单引号字符串 `'...'` → `?`（`''` 视为字符串内转义的单引号）
/// - 数字字面量（整数或小数，且不作为标识符的一部分）→ `?`
///
/// 不替换：
/// - 标识符中的数字，如 `col1`、`table2`
/// - `NULL`、`TRUE`、`FALSE` 等关键字（保持原样）
/// # Panics
///
/// 理论上不会 panic：输出字节序列可以被证明始终是合法 UTF-8（见函数体注释）。
/// `expect` 仅作为内部一致性断言保留。
#[must_use]
pub fn normalize_sql(sql: &str) -> String {
    let bytes = sql.as_bytes();
    let len = bytes.len();
    let mut result = Vec::with_capacity(len);
    let mut i = 0;

    while i < len {
        match bytes[i] {
            b'\'' => {
                // 消费整个字符串字面量，输出单个 ?
                i += 1;
                while i < len {
                    if bytes[i] == b'\'' {
                        i += 1;
                        if i < len && bytes[i] == b'\'' {
                            // '' 是字符串内转义的单引号，继续消费
                            i += 1;
                        } else {
                            break;
                        }
                    } else {
                        i += 1;
                    }
                }
                result.push(b'?');
            }
            b if b.is_ascii_digit() => {
                // 若前一个字符是标识符字符（字母、数字、下划线），则当前数字是标识符的一部分，直接保留
                let prev_is_ident = result
                    .last()
                    .is_some_and(|&p: &u8| p.is_ascii_alphanumeric() || p == b'_');
                if prev_is_ident {
                    result.push(bytes[i]);
                    i += 1;
                } else {
                    // 消费完整的数字（含可选小数点），输出单个 ?
                    while i < len && (bytes[i].is_ascii_digit() || bytes[i] == b'.') {
                        i += 1;
                    }
                    result.push(b'?');
                }
            }
            b => {
                result.push(b);
                i += 1;
            }
        }
    }

    // 安全性：result 中的字节仅来自两个来源：
    // 1. b'?' (0x3F)：合法 ASCII。
    // 2. sql 的原始字节按序复制：
    //    - 单引号 (0x27) 和 ASCII 数字 (0x30–0x39) 均不可能是多字节 UTF-8 的延续字节
    //      （延续字节范围 0x80–0xBF），因此状态机的分支判断不会在多字节序列中间切分。
    //    - 其余字节经 default 分支逐字节复制，多字节序列的所有字节均按原序完整输出。
    // 综上，result 始终构成合法的 UTF-8 字节序列。
    String::from_utf8(result).expect("normalize_sql produced invalid UTF-8")
}

#[cfg(test)]
mod tests {
    use super::normalize_sql;

    #[test]
    fn test_string_literal() {
        assert_eq!(normalize_sql("WHERE name = 'Alice'"), "WHERE name = ?");
    }

    #[test]
    fn test_escaped_quote_in_string() {
        assert_eq!(normalize_sql("WHERE name = 'O''Brien'"), "WHERE name = ?");
    }

    #[test]
    fn test_multiple_strings() {
        assert_eq!(
            normalize_sql("INSERT INTO t VALUES ('a', 'b')"),
            "INSERT INTO t VALUES (?, ?)"
        );
    }

    #[test]
    fn test_integer_literal() {
        assert_eq!(normalize_sql("WHERE id = 42"), "WHERE id = ?");
    }

    #[test]
    fn test_float_literal() {
        assert_eq!(normalize_sql("WHERE score > 3.14"), "WHERE score > ?");
    }

    #[test]
    fn test_identifier_with_number() {
        assert_eq!(
            normalize_sql("SELECT col1, table2 FROM t"),
            "SELECT col1, table2 FROM t"
        );
    }

    #[test]
    fn test_mixed() {
        assert_eq!(
            normalize_sql("SELECT * FROM t WHERE id = 1 AND name = 'Bob' AND score > 9.5"),
            "SELECT * FROM t WHERE id = ? AND name = ? AND score > ?"
        );
    }

    #[test]
    fn test_no_params() {
        assert_eq!(normalize_sql("SELECT * FROM t"), "SELECT * FROM t");
    }

    #[test]
    fn test_chinese_in_string() {
        assert_eq!(normalize_sql("WHERE name = '张三'"), "WHERE name = ?");
    }
}
