use std::collections::HashMap;

/// A single parameter value parsed from a `PARAMS(...)` log record.
#[derive(Debug, Clone)]
pub enum ParamValue {
    /// Single-quoted string already including the surrounding quotes, e.g. `'3USJ29'`.
    Quoted(String),
    /// Bare numeric literal, e.g. `2370075`.
    Bare(String),
    /// NULL, BLOB, or any empty-value entry.
    Null,
}

impl ParamValue {
    fn as_sql(&self) -> &str {
        match self {
            Self::Quoted(s) | Self::Bare(s) => s.as_str(),
            Self::Null => "NULL",
        }
    }
}

/// Parse a `PARAMS(SEQNO, TYPE, DATA)={...}` record body into an ordered list of values.
///
/// Returns `None` if the body does not match the expected format.
#[must_use]
pub fn parse_params(body: &str) -> Option<Vec<ParamValue>> {
    let brace = body.find("={")?;
    let inner = body[brace + 2..].strip_suffix('}')?;

    let mut params = Vec::new();
    let mut rest = inner.trim();

    while !rest.is_empty() {
        let (value, tail) = parse_one_entry(rest)?;
        params.push(value);
        rest = tail.trim();
        if let Some(t) = rest.strip_prefix(',') {
            rest = t.trim();
        }
    }

    Some(params)
}

/// Parse one `(seqno, type, value)` entry from the front of `s`.
/// Returns `(parsed_value, remaining_input)`.
fn parse_one_entry(s: &str) -> Option<(ParamValue, &str)> {
    let s = s.strip_prefix('(')?;

    // Skip SEQNO (integer up to first comma)
    let comma1 = s.find(',')?;
    let s = s[comma1 + 1..].trim_start();

    // Skip TYPE (up to next comma)
    let comma2 = s.find(',')?;
    let s = s[comma2 + 1..].trim_start();

    // Parse VALUE then the closing ')'
    if s.starts_with('\'') {
        // Quoted string — scan forward to the closing unescaped single-quote
        let bytes = s.as_bytes();
        let mut i = 1;
        loop {
            if i >= bytes.len() {
                return None;
            }
            if bytes[i] == b'\'' {
                i += 1;
                // '' is an escaped quote inside the string
                if i < bytes.len() && bytes[i] == b'\'' {
                    i += 1;
                } else {
                    break;
                }
            } else {
                i += 1;
            }
        }
        // s[..i] is the quoted string including both surrounding quotes
        let quoted = &s[..i];
        let tail = s[i..].trim_start().strip_prefix(')')?;
        Some((ParamValue::Quoted(quoted.to_string()), tail))
    } else {
        // Bare number or empty — find closing ')'
        let end = s.find(')')?;
        let raw = s[..end].trim();
        let tail = &s[end + 1..];
        let value = if raw.is_empty() {
            ParamValue::Null
        } else {
            ParamValue::Bare(raw.to_string())
        };
        Some((value, tail))
    }
}

/// Detect which placeholder style the SQL uses and count the number of slots,
/// skipping over single-quoted string literals.
///
/// Returns `(count, is_colon_style)`:
/// - `is_colon_style = false` → `?` style; count = number of `?` outside literals
/// - `is_colon_style = true`  → `:N` Oracle style; count = highest ordinal seen
///
/// If the SQL contains no recognisable placeholders, returns `(0, false)`.
#[must_use]
pub fn count_placeholders(sql: &str) -> (usize, bool) {
    let bytes = sql.as_bytes();
    let len = bytes.len();
    let mut i = 0;
    let mut question_count = 0usize;
    let mut max_colon_ordinal = 0usize;

    while i < len {
        match bytes[i] {
            b'\'' => {
                // Skip string literal verbatim
                i += 1;
                while i < len {
                    if bytes[i] == b'\'' {
                        i += 1;
                        if i < len && bytes[i] == b'\'' {
                            i += 1; // '' escape
                        } else {
                            break;
                        }
                    } else {
                        i += 1;
                    }
                }
            }
            b'?' => {
                question_count += 1;
                i += 1;
            }
            b':' => {
                // `:N` where N is one or more decimal digits
                let start = i + 1;
                let mut j = start;
                while j < len && bytes[j].is_ascii_digit() {
                    j += 1;
                }
                if j > start {
                    if let Some(n) = std::str::from_utf8(&bytes[start..j])
                        .ok()
                        .and_then(|s| s.parse::<usize>().ok())
                    {
                        max_colon_ordinal = max_colon_ordinal.max(n);
                    }
                    i = j;
                } else {
                    i += 1;
                }
            }
            _ => {
                i += 1;
            }
        }
    }

    if max_colon_ordinal > 0 {
        (max_colon_ordinal, true)
    } else {
        (question_count, false)
    }
}

/// Replace parameter placeholders in `sql` with values from `params`.
///
/// Supports two placeholder styles:
/// - `?`  — replaced sequentially: first `?` → `params[0]`, second → `params[1]`, …
/// - `:N` — replaced by ordinal:   `:1` → `params[0]`, `:2` → `params[1]`, …
///
/// String params are already single-quoted (e.g. `'hello'`); numeric and NULL params
/// are written bare or as `NULL`. Placeholders inside single-quoted SQL string literals
/// are never replaced.
///
/// **Callers must verify that `params.len()` equals `count_placeholders(sql).0`
/// before calling this function.**  If counts differ the result is unspecified.
///
/// # Panics
///
/// Will not panic in practice: the output is valid UTF-8 (original SQL bytes plus
/// ASCII param literals). The `expect` is an internal consistency assertion.
#[must_use]
pub fn apply_params(sql: &str, params: &[ParamValue], colon_style: bool) -> String {
    if params.is_empty() {
        return sql.to_string();
    }

    let extra: usize = params
        .iter()
        .map(|p| p.as_sql().len().saturating_sub(1))
        .sum();
    let mut result: Vec<u8> = Vec::with_capacity(sql.len() + extra);
    let bytes = sql.as_bytes();
    let len = bytes.len();
    let mut i = 0;
    let mut seq_idx = 0usize; // used for `?` style

    while i < len {
        match bytes[i] {
            b'\'' => {
                // Copy string literal verbatim
                result.push(b'\'');
                i += 1;
                while i < len {
                    if bytes[i] == b'\'' {
                        result.push(b'\'');
                        i += 1;
                        if i < len && bytes[i] == b'\'' {
                            result.push(b'\'');
                            i += 1;
                        } else {
                            break;
                        }
                    } else {
                        result.push(bytes[i]);
                        i += 1;
                    }
                }
            }
            b'?' if !colon_style => {
                if let Some(p) = params.get(seq_idx) {
                    result.extend_from_slice(p.as_sql().as_bytes());
                } else {
                    result.push(b'?');
                }
                seq_idx += 1;
                i += 1;
            }
            b':' if colon_style => {
                let start = i + 1;
                let mut j = start;
                while j < len && bytes[j].is_ascii_digit() {
                    j += 1;
                }
                if j > start {
                    let n: usize = std::str::from_utf8(&bytes[start..j])
                        .unwrap_or("0")
                        .parse()
                        .unwrap_or(0);
                    // :N is 1-indexed
                    if let Some(p) = n.checked_sub(1).and_then(|idx| params.get(idx)) {
                        result.extend_from_slice(p.as_sql().as_bytes());
                    } else {
                        result.extend_from_slice(&bytes[i..j]);
                    }
                    i = j;
                } else {
                    result.push(b':');
                    i += 1;
                }
            }
            b => {
                result.push(b);
                i += 1;
            }
        }
    }

    // Safety: bytes are either verbatim UTF-8 from sql, or ASCII param literals.
    // b'\'' (0x27) and b'?' (0x3F) and b':' (0x3A) are all ASCII and cannot be
    // UTF-8 continuation bytes (0x80–0xBF), so multi-byte sequences are preserved.
    String::from_utf8(result).expect("apply_params produced invalid UTF-8")
}

/// Helper used in `cli/run.rs` to update the params buffer and compute the
/// `normalized_sql` value for a single log record.
///
/// - If the record is a `PARAMS(...)` record, its values are stored in `buffer`
///   (keyed by `(trxid, stmt)`) and `None` is returned.
/// - If the record is an `[INS]`/`[DEL]`/`[UPD]`/`[SEL]` execution record that
///   has a matching entry in `buffer`, the SQL with substituted parameters is
///   returned as `Some(String)`.
/// - For all other records, `None` is returned.
///
/// `placeholder_override`:
/// - `None`        → auto-detect from the SQL (`:N` takes priority over `?`)
/// - `Some(true)`  → force colon-style (`:N`)
/// - `Some(false)` → force question-style (`?`)
pub fn compute_normalized<S: std::hash::BuildHasher>(
    record: &dm_database_parser_sqllog::Sqllog<'_>,
    buffer: &mut HashMap<(String, String), Vec<ParamValue>, S>,
    placeholder_override: Option<bool>,
) -> Option<String> {
    let body = record.body();

    // PARAMS record: buffer the values, produce no output
    if record.tag.is_none() && body.starts_with("PARAMS(") {
        let meta = record.parse_meta();
        if let Some(params) = parse_params(body.as_ref()) {
            buffer.insert((meta.trxid.to_string(), meta.statement.to_string()), params);
        }
        return None;
    }

    // DML/SEL execution record
    let tag = record.tag.as_deref()?;
    if !matches!(tag, "INS" | "DEL" | "UPD" | "SEL") {
        return None;
    }

    // 先扫描 SQL 是否含占位符，大多数 SQL 没有占位符，可以提前返回，
    // 避免 parse_meta() 调用和两次 String 分配（trxid + statement key）。
    let pm = record.parse_performance_metrics();
    let sql = pm.sql.as_ref();

    let (placeholder_count, detected_colon) = count_placeholders(sql);
    if placeholder_count == 0 {
        return None;
    }

    let meta = record.parse_meta();
    let key = (meta.trxid.to_string(), meta.statement.to_string());

    // 消耗 buffer 条目：每个 PARAMS 只对应紧跟其后的一次执行
    let params = buffer.remove(&key)?;

    let colon_style = placeholder_override.unwrap_or(detected_colon);

    if params.len() != placeholder_count {
        log::warn!(
            "replace_parameters: param count mismatch (params={}, placeholders={}) for sql: {}",
            params.len(),
            placeholder_count,
            &sql[..sql.len().min(120)]
        );
        return None;
    }

    Some(apply_params(sql, &params, colon_style))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bare(s: &str) -> ParamValue {
        ParamValue::Bare(s.to_string())
    }
    fn quoted(s: &str) -> ParamValue {
        ParamValue::Quoted(s.to_string())
    }

    // ── parse_params ──────────────────────────────────────────────────────────

    #[test]
    fn test_parse_single_varchar() {
        let params = parse_params("PARAMS(SEQNO, TYPE, DATA)={(0, VARCHAR, 'SM')}").unwrap();
        assert_eq!(params.len(), 1);
        assert_eq!(params[0].as_sql(), "'SM'");
    }

    #[test]
    fn test_parse_mixed_types() {
        let params = parse_params(
            "PARAMS(SEQNO, TYPE, DATA)={(0, DEC, 3), (1, VARCHAR, 'send ok'), (2, DEC, 0), (3, INTEGER, 42)}",
        )
        .unwrap();
        assert_eq!(params.len(), 4);
        assert_eq!(params[0].as_sql(), "3");
        assert_eq!(params[1].as_sql(), "'send ok'");
        assert_eq!(params[2].as_sql(), "0");
        assert_eq!(params[3].as_sql(), "42");
    }

    #[test]
    fn test_parse_blob_empty() {
        let params = parse_params("PARAMS(SEQNO, TYPE, DATA)={(0, DEC, 1), (1, BLOB, )}").unwrap();
        assert_eq!(params.len(), 2);
        assert_eq!(params[0].as_sql(), "1");
        assert_eq!(params[1].as_sql(), "NULL");
    }

    #[test]
    fn test_parse_quoted_with_escaped_quote() {
        let params = parse_params("PARAMS(SEQNO, TYPE, DATA)={(0, VARCHAR, 'O''Brien')}").unwrap();
        assert_eq!(params[0].as_sql(), "'O''Brien'");
    }

    #[test]
    fn test_parse_invalid_returns_none() {
        assert!(parse_params("not a params record").is_none());
    }

    // ── apply_params ──────────────────────────────────────────────────────────

    #[test]
    fn test_apply_single_string_param() {
        let params = vec![quoted("'3USJ29'")];
        let result = apply_params("WHERE code = ?", &params, false);
        assert_eq!(result, "WHERE code = '3USJ29'");
    }

    #[test]
    fn test_apply_numeric_param() {
        let params = vec![bare("42")];
        let result = apply_params("WHERE id = ?", &params, false);
        assert_eq!(result, "WHERE id = 42");
    }

    #[test]
    fn test_apply_null_param() {
        let params = vec![ParamValue::Null];
        let result = apply_params("WHERE tag = ?", &params, false);
        assert_eq!(result, "WHERE tag = NULL");
    }

    #[test]
    fn test_apply_multiple_params() {
        let params = vec![bare("2370075"), quoted("'SJ-1'"), ParamValue::Null];
        let result = apply_params("VALUES (?, ?, ?)", &params, false);
        assert_eq!(result, "VALUES (2370075, 'SJ-1', NULL)");
    }

    #[test]
    fn test_apply_no_placeholders() {
        let params = vec![bare("1")];
        let result = apply_params("SELECT 1", &params, false);
        assert_eq!(result, "SELECT 1");
    }

    #[test]
    fn test_apply_skip_literal_contents() {
        // The '?' inside the string literal should NOT be replaced
        let params = vec![quoted("'real'")];
        let result = apply_params("WHERE a = '?' AND b = ?", &params, false);
        assert_eq!(result, "WHERE a = '?' AND b = 'real'");
    }

    #[test]
    fn test_apply_insert_with_function() {
        // current_timestamp is not a placeholder; only the bare ? are replaced
        let params = vec![bare("1"), quoted("'hello'"), bare("99")];
        let result = apply_params(
            "INSERT INTO t VALUES (?,current_timestamp,?,?)",
            &params,
            false,
        );
        assert_eq!(
            result,
            "INSERT INTO t VALUES (1,current_timestamp,'hello',99)"
        );
    }

    #[test]
    fn test_apply_chinese_in_param() {
        let params = vec![quoted("'张三'")];
        let result = apply_params("WHERE name = ?", &params, false);
        assert_eq!(result, "WHERE name = '张三'");
    }

    // ── colon-style placeholders ───────────────────────────────────────────────

    #[test]
    fn test_apply_colon_style_basic() {
        let params = vec![bare("10"), quoted("'abc'")];
        let result = apply_params("WHERE id = :1 AND code = :2", &params, true);
        assert_eq!(result, "WHERE id = 10 AND code = 'abc'");
    }

    #[test]
    fn test_apply_colon_style_out_of_order() {
        let params = vec![bare("1"), bare("2"), bare("3")];
        let result = apply_params("SELECT :3, :1, :2", &params, true);
        assert_eq!(result, "SELECT 3, 1, 2");
    }

    #[test]
    fn test_count_placeholders_question() {
        let (count, colon_style) = count_placeholders("WHERE a = ? AND b = ?");
        assert_eq!(count, 2);
        assert!(!colon_style);
    }

    #[test]
    fn test_count_placeholders_colon() {
        let (count, colon_style) = count_placeholders("WHERE a = :1 AND b = :2 AND c = :3");
        assert_eq!(count, 3);
        assert!(colon_style);
    }

    #[test]
    fn test_count_placeholders_skips_literals() {
        let (count, colon_style) = count_placeholders("WHERE a = '?' AND b = ?");
        assert_eq!(count, 1);
        assert!(!colon_style);
    }

    #[test]
    fn test_count_placeholders_none() {
        let (count, colon_style) = count_placeholders("SELECT 1");
        assert_eq!(count, 0);
        assert!(!colon_style);
    }
}
