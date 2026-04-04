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

/// Replace `?` placeholders in `sql` with the corresponding parameter values in order.
///
/// - String params: output as-is (already single-quoted, e.g. `'hello'`)
/// - Numeric params: output as bare literal
/// - NULL/BLOB/empty params: output as `NULL`
///
/// `?` that appear inside single-quoted string literals in `sql` are not replaced.
///
/// # Panics
///
/// Will not panic in practice: the output bytes are either verbatim UTF-8 from
/// `sql` or ASCII characters from param values. The `expect` is an internal
/// consistency assertion.
#[must_use]
pub fn apply_params(sql: &str, params: &[ParamValue]) -> String {
    if params.is_empty() || !sql.contains('?') {
        return sql.to_string();
    }

    let extra: usize = params
        .iter()
        .map(|p| p.as_sql().len().saturating_sub(1))
        .sum();
    let mut result: Vec<u8> = Vec::with_capacity(sql.len() + extra);
    let mut param_idx = 0;
    let bytes = sql.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i < len {
        match bytes[i] {
            b'\'' => {
                // Copy the string literal verbatim — don't substitute ? inside it
                result.push(b'\'');
                i += 1;
                while i < len {
                    if bytes[i] == b'\'' {
                        result.push(b'\'');
                        i += 1;
                        if i < len && bytes[i] == b'\'' {
                            // '' escape inside the literal
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
            b'?' => {
                if let Some(p) = params.get(param_idx) {
                    result.extend_from_slice(p.as_sql().as_bytes());
                } else {
                    result.push(b'?'); // no param available — keep placeholder
                }
                param_idx += 1;
                i += 1;
            }
            b => {
                result.push(b);
                i += 1;
            }
        }
    }

    // Safety: result bytes come from:
    // 1. Original sql bytes (UTF-8 preserved, including multi-byte sequences)
    // 2. ASCII param literals (numbers, NULL, single-quoted ASCII strings)
    // The SQL scanning never cuts across multi-byte sequences because b'\'' (0x27)
    // and b'?' (0x3F) are both ASCII and cannot appear as UTF-8 continuation bytes
    // (range 0x80–0xBF).
    String::from_utf8(result).expect("apply_params produced invalid UTF-8")
}

/// Helper used in `cli/run.rs` to update the params buffer and compute the
/// `normalized_sql` value for a single log record.
///
/// - If the record is a `PARAMS(...)` record, its values are stored in `buffer`
///   (keyed by `(trxid, stmt)`) and `None` is returned.
/// - If the record is an `[INS]`/`[DEL]`/`[UPD]`/`[ORA]` execution record that
///   has a matching entry in `buffer`, the SQL with substituted parameters is
///   returned as `Some(String)`.
/// - For all other records, `None` is returned.
pub fn compute_normalized<S: std::hash::BuildHasher>(
    record: &dm_database_parser_sqllog::Sqllog<'_>,
    buffer: &mut HashMap<(String, String), Vec<ParamValue>, S>,
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

    // DML/ORA execution record
    let tag = record.tag.as_deref()?;
    if !matches!(tag, "INS" | "DEL" | "UPD" | "ORA") {
        return None;
    }

    let meta = record.parse_meta();
    let key = (meta.trxid.to_string(), meta.statement.to_string());
    let params = buffer.get(&key)?;

    let pm = record.parse_performance_metrics();
    let sql = pm.sql.as_ref();

    if !sql.contains('?') {
        return None;
    }

    Some(apply_params(sql, params))
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
        let result = apply_params("WHERE code = ?", &params);
        assert_eq!(result, "WHERE code = '3USJ29'");
    }

    #[test]
    fn test_apply_numeric_param() {
        let params = vec![bare("42")];
        let result = apply_params("WHERE id = ?", &params);
        assert_eq!(result, "WHERE id = 42");
    }

    #[test]
    fn test_apply_null_param() {
        let params = vec![ParamValue::Null];
        let result = apply_params("WHERE tag = ?", &params);
        assert_eq!(result, "WHERE tag = NULL");
    }

    #[test]
    fn test_apply_multiple_params() {
        let params = vec![bare("2370075"), quoted("'SJ-1'"), ParamValue::Null];
        let result = apply_params("VALUES (?, ?, ?)", &params);
        assert_eq!(result, "VALUES (2370075, 'SJ-1', NULL)");
    }

    #[test]
    fn test_apply_no_placeholders() {
        let params = vec![bare("1")];
        let result = apply_params("SELECT 1", &params);
        assert_eq!(result, "SELECT 1");
    }

    #[test]
    fn test_apply_skip_literal_contents() {
        // The '?' inside the string literal should NOT be replaced
        let params = vec![quoted("'real'")];
        let result = apply_params("WHERE a = '?' AND b = ?", &params);
        assert_eq!(result, "WHERE a = '?' AND b = 'real'");
    }

    #[test]
    fn test_apply_insert_with_function() {
        // current_timestamp is not a placeholder; only the bare ? are replaced
        let params = vec![bare("1"), quoted("'hello'"), bare("99")];
        let result = apply_params("INSERT INTO t VALUES (?,current_timestamp,?,?)", &params);
        assert_eq!(
            result,
            "INSERT INTO t VALUES (1,current_timestamp,'hello',99)"
        );
    }

    #[test]
    fn test_apply_chinese_in_param() {
        let params = vec![quoted("'张三'")];
        let result = apply_params("WHERE name = ?", &params);
        assert_eq!(result, "WHERE name = '张三'");
    }
}
