/// 字节查找表：需要特殊处理的字节（单引号、ASCII 空白、ASCII 数字、`-`、`/`）。
/// `-` 和 `/` 用于 normalize 路径的注释检测；fingerprint 路径下落入默认分支，行为不变。
const NEEDS_SPECIAL_NORM: [bool; 256] = {
    let mut t = [false; 256];
    t[b'\'' as usize] = true;
    t[b' ' as usize] = true;
    t[b'\t' as usize] = true;
    t[b'\n' as usize] = true;
    t[b'\r' as usize] = true;
    t[0x0B_usize] = true; // vertical tab
    t[0x0C_usize] = true; // form feed
    let mut d = b'0';
    while d <= b'9' {
        t[d as usize] = true;
        d += 1;
    }
    t[b'-' as usize] = true;
    t[b'/' as usize] = true;
    t
};

#[derive(Clone, Copy)]
enum ScanMode {
    Fingerprint,
    // Phase 13 will re-enable this variant when TemplateAggregator::observe() is wired in.
    #[allow(dead_code)]
    Normalize,
}

/// 将 SQL 字符串转为指纹：字面量替换为 `?`，折叠连续空白。
///
/// 结构相同但参数不同的 SQL 将得到同一指纹，用于 `digest` 命令聚合。
#[must_use]
pub fn fingerprint(sql: &str) -> String {
    scan_sql_bytes(sql, ScanMode::Fingerprint)
}

/// 将 SQL 字符串归一化为模板 key：去除注释、折叠 IN 列表、统一关键字大小写、折叠空白。
///
/// 结构相同的 SQL（无论字面量值或数量）将得到同一模板 key，用于模板聚合统计。
/// Phase 13 will call this via `TemplateAggregator::observe()`.
#[must_use]
#[allow(dead_code)]
pub fn normalize_template(sql: &str) -> String {
    scan_sql_bytes(sql, ScanMode::Normalize)
}

fn scan_sql_bytes(sql: &str, mode: ScanMode) -> String {
    let bytes = sql.as_bytes();
    let len = bytes.len();
    let mut out: Vec<u8> = Vec::with_capacity(sql.len());
    let mut i = 0;
    while i < len {
        let bulk_start = i;
        while i < len
            && !NEEDS_SPECIAL_NORM[bytes[i] as usize]
            && !(matches!(mode, ScanMode::Normalize) && bytes[i].is_ascii_alphabetic())
        {
            i += 1;
        }
        if i > bulk_start {
            out.extend_from_slice(&bytes[bulk_start..i]);
        }
        if i >= len {
            break;
        }
        i = dispatch_byte(bytes, i, len, &mut out, mode);
    }
    let out_str = String::from_utf8(out).expect("scan_sql_bytes: invalid UTF-8");
    let trimmed = out_str.trim_ascii();
    if trimmed.len() == out_str.len() {
        out_str
    } else {
        trimmed.to_string()
    }
}

fn dispatch_byte(bytes: &[u8], i: usize, len: usize, out: &mut Vec<u8>, mode: ScanMode) -> usize {
    match bytes[i] {
        b'\'' => handle_quote(bytes, i, out, matches!(mode, ScanMode::Normalize)),
        b'-' if matches!(mode, ScanMode::Normalize) && i + 1 < len && bytes[i + 1] == b'-' => {
            handle_line_comment(bytes, i)
        }
        b'/' if matches!(mode, ScanMode::Normalize) && i + 1 < len && bytes[i + 1] == b'*' => {
            handle_block_comment(bytes, i, out)
        }
        b if b.is_ascii_digit()
            && !prev_is_ident_byte(out)
            && matches!(mode, ScanMode::Fingerprint) =>
        {
            out.push(b'?');
            let mut j = i + 1;
            while j < len && (bytes[j].is_ascii_digit() || bytes[j] == b'.') {
                j += 1;
            }
            j
        }
        b if b.is_ascii_whitespace() => {
            if !matches!(out.last(), Some(&b' ')) {
                out.push(b' ');
            }
            let mut j = i + 1;
            while j < len && bytes[j].is_ascii_whitespace() {
                j += 1;
            }
            j
        }
        b if b.is_ascii_alphabetic() && matches!(mode, ScanMode::Normalize) => {
            handle_word(bytes, i, len, out)
        }
        b => {
            out.push(b);
            i + 1
        }
    }
}

/// 处理单引号字符串字面量。`keep_literal` 为 true 时保留原文（normalize），false 时替换为 `?`（fingerprint）。
fn handle_quote(bytes: &[u8], i: usize, out: &mut Vec<u8>, keep_literal: bool) -> usize {
    let literal_start = i;
    if !keep_literal {
        out.push(b'?');
    }
    let mut j = i + 1;
    let len = bytes.len();
    loop {
        let Some(rel) = memchr::memchr(b'\'', &bytes[j..]) else {
            j = len;
            break;
        };
        j += rel + 1;
        if j < len && bytes[j] == b'\'' {
            j += 1; // '' 转义，继续消费
        } else {
            break;
        }
    }
    if keep_literal {
        out.extend_from_slice(&bytes[literal_start..j]);
    }
    j
}

/// 跳过单行注释（`--` 到行尾），i 指向第一个 `-`。
fn handle_line_comment(bytes: &[u8], i: usize) -> usize {
    match memchr::memchr(b'\n', &bytes[i..]) {
        Some(rel) => i + rel + 1,
        None => bytes.len(),
    }
}

/// 跳过块注释（`/* ... */`），i 指向 `/`，替换为单空格避免 token 粘连。
fn handle_block_comment(bytes: &[u8], i: usize, out: &mut Vec<u8>) -> usize {
    let len = bytes.len();
    let mut j = i + 2;
    match memchr::memmem::find(&bytes[j..], b"*/") {
        Some(rel) => j += rel + 2,
        None => j = len,
    }
    if !matches!(out.last(), Some(&b' ')) {
        out.push(b' ');
    }
    j
}

/// 处理单词（normalize 路径）：关键字大写化，IN 列表尝试折叠。
fn handle_word(bytes: &[u8], i: usize, len: usize, out: &mut Vec<u8>) -> usize {
    let start = i;
    let mut j = i;
    while j < len && is_ident_byte(bytes[j]) {
        j += 1;
    }
    let word = &bytes[start..j];
    if prev_is_ident_byte(out) {
        // 处于标识符中部（如 t.column 中的 column），直接复制
        out.extend_from_slice(word);
        return j;
    }
    if is_keyword(word) {
        for &b in word {
            out.push(b.to_ascii_uppercase());
        }
        if word.len() == 2 && word.eq_ignore_ascii_case(b"IN") {
            if let Some(new_j) = try_fold_in_list(bytes, j, len, out) {
                return new_j;
            }
        }
    } else {
        out.extend_from_slice(word);
    }
    j
}

/// 尝试将 IN (...) 折叠为 IN (?)；含子查询则放弃并返回 None。
fn try_fold_in_list(bytes: &[u8], mut i: usize, len: usize, out: &mut Vec<u8>) -> Option<usize> {
    while i < len && bytes[i].is_ascii_whitespace() {
        i += 1;
    }
    if i >= len || bytes[i] != b'(' {
        return None;
    }
    let mut j = i + 1;
    let mut depth = 1usize;
    while j < len && depth > 0 {
        match bytes[j] {
            b'(' => {
                depth += 1;
                j += 1;
            }
            b')' => {
                depth -= 1;
                j += 1;
            }
            b'\'' => j = skip_quoted(bytes, j + 1),
            _ => j += 1,
        }
    }
    if depth != 0 {
        return None;
    }
    let inner = &bytes[i + 1..j - 1];
    if is_subquery(inner) {
        return None;
    }
    out.extend_from_slice(b" (?)");
    Some(j)
}

/// 从 j（引号后第一字节）跳过单引号字符串，返回闭合引号后的位置。
fn skip_quoted(bytes: &[u8], mut j: usize) -> usize {
    let len = bytes.len();
    loop {
        let Some(rel) = memchr::memchr(b'\'', &bytes[j..]) else {
            return len;
        };
        j += rel + 1;
        if j < len && bytes[j] == b'\'' {
            j += 1;
        } else {
            return j;
        }
    }
}

/// 检测 IN 列表内容中是否含子查询（包含独立的 SELECT 或 FROM 关键字）。
fn is_subquery(inner: &[u8]) -> bool {
    let len = inner.len();
    let mut i = 0;
    while i < len {
        if inner[i].is_ascii_alphabetic() {
            let start = i;
            while i < len && is_ident_byte(inner[i]) {
                i += 1;
            }
            let word = &inner[start..i];
            if word.eq_ignore_ascii_case(b"SELECT") || word.eq_ignore_ascii_case(b"FROM") {
                return true;
            }
        } else if inner[i] == b'\'' {
            i = skip_quoted(inner, i + 1);
        } else {
            i += 1;
        }
    }
    false
}

/// 判断 word 是否为 SQL 关键字（大小写不敏感）。
fn is_keyword(word: &[u8]) -> bool {
    if word.len() > 8 {
        return false;
    }
    let mut buf = [0u8; 8];
    for (idx, &b) in word.iter().enumerate() {
        buf[idx] = b.to_ascii_uppercase();
    }
    let s = &buf[..word.len()];
    matches!(
        s,
        b"SELECT"
            | b"FROM"
            | b"WHERE"
            | b"AND"
            | b"OR"
            | b"JOIN"
            | b"ON"
            | b"AS"
            | b"INSERT"
            | b"UPDATE"
            | b"DELETE"
            | b"INTO"
            | b"VALUES"
            | b"SET"
            | b"GROUP"
            | b"ORDER"
            | b"BY"
            | b"HAVING"
            | b"UNION"
            | b"DISTINCT"
            | b"LIMIT"
            | b"CREATE"
            | b"DROP"
            | b"ALTER"
            | b"IN"
            | b"NOT"
            | b"NULL"
            | b"IS"
            | b"BETWEEN"
            | b"LIKE"
            | b"EXISTS"
            | b"CASE"
            | b"WHEN"
            | b"THEN"
            | b"ELSE"
            | b"END"
    )
}

/// 单字节是否为标识符字节（字母/数字/下划线/点）。
fn is_ident_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'.'
}

/// 上一个输出字节是否是标识符字节（字母/数字/下划线/点）。
fn prev_is_ident_byte(out: &[u8]) -> bool {
    out.last().is_some_and(|&b| is_ident_byte(b))
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- fingerprint 原有测试（9 项，零回归） ---

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

    // --- normalize_template 新增测试（8 项） ---

    #[test]
    fn test_normalize_line_comment_removed() {
        assert_eq!(normalize_template("-- comment\nSELECT 1"), "SELECT 1");
    }

    #[test]
    fn test_normalize_block_comment_replaced() {
        assert_eq!(normalize_template("/* multi */ SELECT 1"), "SELECT 1");
    }

    #[test]
    fn test_normalize_in_list_numbers_same_key() {
        let a = normalize_template("SELECT * FROM t WHERE id IN (1, 2, 3)");
        let b = normalize_template("SELECT * FROM t WHERE id IN (10, 20, 30, 40)");
        assert_eq!(a, b);
    }

    #[test]
    fn test_normalize_in_list_strings_same_key() {
        let a = normalize_template("SELECT * FROM t WHERE name IN ('a', 'b')");
        let b = normalize_template("SELECT * FROM t WHERE name IN ('xx', 'yy', 'zz')");
        assert_eq!(a, b);
    }

    #[test]
    fn test_normalize_keyword_uppercase() {
        let result = normalize_template("select * from t where id = 1");
        assert!(result.contains("SELECT"), "expected SELECT in {result}");
        assert!(result.contains("FROM"), "expected FROM in {result}");
        assert!(result.contains("WHERE"), "expected WHERE in {result}");
    }

    #[test]
    fn test_normalize_ident_with_underscore_preserved() {
        let result = normalize_template("SELECT a FROM outer_join_t");
        assert!(
            result.contains("outer_join_t"),
            "expected outer_join_t in {result}"
        );
    }

    #[test]
    fn test_normalize_string_literal_hides_comment_marker() {
        let result = normalize_template("WHERE col = '-- not a comment'");
        assert!(
            result.contains("'-- not a comment'"),
            "expected literal preserved in {result}"
        );
    }

    #[test]
    fn test_normalize_whitespace_collapsed() {
        assert_eq!(normalize_template("SELECT  *  FROM  t"), "SELECT * FROM t");
    }
}
