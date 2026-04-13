use compact_str::CompactString;
use smallvec::SmallVec;
use std::collections::HashMap;

/// 参数替换缓冲区类型：keyed by (trxid, stmt)，value 为解析好的参数列表。
///
/// - Key 使用 `CompactString`：trxid 和 stmt 通常 ≤23 字节，内联存储，无堆分配。
/// - Value 使用 `SmallVec<[ParamValue; 6]>`：≤6 个参数时不分配堆内存。
pub type ParamBuffer = ahash::HashMap<(CompactString, CompactString), SmallVec<[ParamValue; 6]>>;

/// A single parameter value parsed from a `PARAMS(...)` log record.
///
/// `CompactString` stores strings ≤ 24 bytes inline (no heap allocation),
/// which covers virtually all numeric literals and short string params.
#[derive(Debug, Clone)]
pub enum ParamValue {
    /// Single-quoted string already including the surrounding quotes, e.g. `'3USJ29'`.
    Quoted(CompactString),
    /// Bare numeric literal, e.g. `2370075`.
    Bare(CompactString),
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
///
/// Uses `SmallVec<[ParamValue; 6]>` to avoid heap allocation for typical param lists (≤6 values).
#[must_use]
pub fn parse_params(body: &str) -> Option<SmallVec<[ParamValue; 6]>> {
    // memmem 使用 Two-Way + SIMD 算法，比 str::find 快
    let brace = memchr::memmem::find(body.as_bytes(), b"={")?;
    let inner = body[brace + 2..].strip_suffix('}')?;

    let mut params = SmallVec::new();
    // trim_start：只需去除前导空格，尾部空格在下一次迭代自然消耗
    let mut rest = inner.trim_start();

    while !rest.is_empty() {
        let (value, tail) = parse_one_entry(rest)?;
        params.push(value);
        rest = tail.trim_start();
        if let Some(t) = rest.strip_prefix(',') {
            rest = t.trim_start();
        }
    }

    Some(params)
}

/// Parse one `(seqno, type, value)` entry from the front of `s`.
/// Returns `(parsed_value, remaining_input)`.
fn parse_one_entry(s: &str) -> Option<(ParamValue, &str)> {
    let s = s.strip_prefix('(')?;

    // Skip SEQNO (integer up to first comma) — memchr for SIMD acceleration
    let comma1 = memchr::memchr(b',', s.as_bytes())?;
    let s = s[comma1 + 1..].trim_start();

    // Skip TYPE (up to next comma)
    let comma2 = memchr::memchr(b',', s.as_bytes())?;
    let s = s[comma2 + 1..].trim_start();

    // Parse VALUE then the closing ')'
    if s.starts_with('\'') {
        // Quoted string — use memchr to skip to the next single-quote, same pattern as
        // count_placeholders / apply_params, avoiding the byte-by-byte inner loop.
        let bytes = s.as_bytes();
        let mut i = 1;
        loop {
            let rel = memchr::memchr(b'\'', &bytes[i..])?;
            i += rel + 1;
            // '' is an escaped quote inside the string — consume both and keep scanning
            if i < bytes.len() && bytes[i] == b'\'' {
                i += 1;
            } else {
                break;
            }
        }
        // s[..i] is the quoted string including both surrounding quotes
        let quoted = &s[..i];
        let tail = s[i..].trim_start().strip_prefix(')')?;
        Some((ParamValue::Quoted(CompactString::new(quoted)), tail))
    } else {
        // Bare number or empty — memchr for closing ')'
        let end = memchr::memchr(b')', s.as_bytes())?;
        let raw = s[..end].trim();
        let tail = &s[end + 1..];
        let value = if raw.is_empty() {
            ParamValue::Null
        } else {
            ParamValue::Bare(CompactString::new(raw))
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
#[inline]
#[must_use]
pub fn count_placeholders(sql: &str) -> (usize, bool) {
    let bytes = sql.as_bytes();
    let len = bytes.len();
    let mut i = 0;
    let mut question_count = 0usize;
    let mut max_colon_ordinal = 0usize;

    while i < len {
        // 用 memchr3 跳过无关字节，直接定位到下一个特殊字符
        let Some(rel) = memchr::memchr3(b'\'', b'?', b':', &bytes[i..]) else {
            break; // 无更多特殊字节
        };
        i += rel;

        match bytes[i] {
            b'\'' => {
                // Skip string literal verbatim — use memchr to jump to next quote
                i += 1;
                loop {
                    let Some(r) = memchr::memchr(b'\'', &bytes[i..]) else {
                        i = len;
                        break;
                    };
                    i += r + 1;
                    if i < len && bytes[i] == b'\'' {
                        i += 1; // '' escape, keep scanning
                    } else {
                        break;
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
                while j < bytes.len() && bytes[j].is_ascii_digit() {
                    j += 1;
                }
                if j > start {
                    // `:N` 内的字节均为 ASCII 数字（已 while 保证），直接累加避免 from_utf8 + parse 开销
                    let n: usize = bytes[start..j]
                        .iter()
                        .fold(0usize, |acc, &b| acc * 10 + (b - b'0') as usize);
                    max_colon_ordinal = max_colon_ordinal.max(n);
                    i = j;
                } else {
                    i += 1;
                }
            }
            _ => unreachable!(),
        }
    }

    if max_colon_ordinal > 0 {
        (max_colon_ordinal, true)
    } else {
        (question_count, false)
    }
}

/// Replace parameter placeholders in `sql` with values from `params`, writing
/// the result into `out` (which is cleared first).
///
/// Internal hot-path used by both `apply_params` and [`compute_normalized`].
/// Avoids a `String` allocation when the caller already owns a reusable `Vec<u8>`.
///
/// # Safety invariant
/// `out` will contain valid UTF-8 on return: all bytes are either taken verbatim
/// from `sql` (already valid UTF-8) or are ASCII literals from params.
/// ASCII bytes (0x00–0x7F) can never appear in the interior of a multi-byte
/// UTF-8 sequence (continuation bytes are 0x80–0xBF), so no sequence is broken.
#[inline]
fn apply_params_into(sql: &str, params: &[ParamValue], colon_style: bool, out: &mut Vec<u8>) {
    out.clear();
    if params.is_empty() {
        out.extend_from_slice(sql.as_bytes());
        return;
    }

    let extra: usize = params
        .iter()
        .map(|p| p.as_sql().len().saturating_sub(1))
        .sum();
    out.reserve(sql.len() + extra);
    let bytes = sql.as_bytes();
    let len = bytes.len();
    let mut i = 0;
    let mut seq_idx = 0usize; // used for `?` style

    while i < len {
        // 用 memchr2 跳过无关字节：问号模式找 ' 和 ?，冒号模式找 ' 和 :
        let special = if colon_style {
            memchr::memchr2(b'\'', b':', &bytes[i..])
        } else {
            memchr::memchr2(b'\'', b'?', &bytes[i..])
        };
        let Some(rel) = special else {
            out.extend_from_slice(&bytes[i..]);
            break;
        };
        // 批量复制特殊字节之前的普通内容
        if rel > 0 {
            out.extend_from_slice(&bytes[i..i + rel]);
        }
        i += rel;

        match bytes[i] {
            b'\'' => {
                // Copy string literal verbatim — use memchr to bulk-copy chunks between quotes
                out.push(b'\'');
                i += 1;
                loop {
                    let Some(r) = memchr::memchr(b'\'', &bytes[i..]) else {
                        out.extend_from_slice(&bytes[i..]);
                        i = len;
                        break;
                    };
                    out.extend_from_slice(&bytes[i..=(i + r)]); // copy up to and including the '
                    i += r + 1;
                    if i < len && bytes[i] == b'\'' {
                        out.push(b'\''); // '' escape: emit second '
                        i += 1;
                    } else {
                        break;
                    }
                }
            }
            b'?' if !colon_style => {
                if let Some(p) = params.get(seq_idx) {
                    out.extend_from_slice(p.as_sql().as_bytes());
                } else {
                    out.push(b'?');
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
                    // `:N` 内的字节均为 ASCII 数字，直接累加避免 from_utf8 + parse 开销
                    let n: usize = bytes[start..j]
                        .iter()
                        .fold(0usize, |acc, &b| acc * 10 + (b - b'0') as usize);
                    // :N is 1-indexed
                    if let Some(p) = n.checked_sub(1).and_then(|idx| params.get(idx)) {
                        out.extend_from_slice(p.as_sql().as_bytes());
                    } else {
                        out.extend_from_slice(&bytes[i..j]);
                    }
                    i = j;
                } else {
                    out.push(b':');
                    i += 1;
                }
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
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
#[cfg(test)]
fn apply_params(sql: &str, params: &[ParamValue], colon_style: bool) -> String {
    let mut buf = Vec::new();
    apply_params_into(sql, params, colon_style, &mut buf);
    String::from_utf8(buf).expect("apply_params produced invalid UTF-8")
}

/// Helper used in `cli/run.rs` to update the params buffer and compute the
/// `normalized_sql` value for a single log record.
///
/// Accepts pre-parsed `meta` and `pm_sql` to avoid re-parsing inside this
/// function. For PARAMS records `pm_sql` equals the record body (the two are
/// identical when there are no performance indicators). For DML records it is
/// the SQL statement extracted from `PerformanceMetrics::sql`.
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
///
/// `scratch` is a caller-owned reusable buffer. On a successful substitution the
/// result is written there and a `&str` pointing into it is returned, eliminating
/// a per-record heap allocation. The caller must not modify `scratch` while the
/// returned reference is live.
///
/// # Panics
///
/// Returns `None` only if the result contains bytes that are neither valid UTF-8 nor
/// valid GB18030 (extremely rare). For GB18030 files, the result is automatically
/// transcoded to UTF-8.
pub fn compute_normalized<'a, S: std::hash::BuildHasher>(
    record: &dm_database_parser_sqllog::Sqllog<'_>,
    meta: &dm_database_parser_sqllog::MetaParts<'_>,
    pm_sql: &str,
    buffer: &mut HashMap<(CompactString, CompactString), SmallVec<[ParamValue; 6]>, S>,
    placeholder_override: Option<bool>,
    scratch: &'a mut Vec<u8>,
) -> Option<&'a str> {
    if record.tag.is_none() {
        // 无 tag → 可能是 PARAMS 记录。
        // pm_sql 对于 PARAMS 记录等价于 body()（无性能指标时两者相同），
        // 直接复用，节省一次 find_indicators_split() 后向扫描。
        if pm_sql.starts_with("PARAMS(") {
            if let Some(params) = parse_params(pm_sql) {
                // CompactString 对短字符串（≤23 字节）内联存储，消除堆分配。
                // trxid（如 "12345"）和 statement（如 "0x1"）通常都满足此条件。
                buffer.insert(
                    (
                        CompactString::from(meta.trxid.as_ref()),
                        CompactString::from(meta.statement.as_ref()),
                    ),
                    params,
                );
            }
        }
        return None;
    }

    // 有 tag → DML/SEL 执行记录
    let tag = record.tag.as_deref()?;
    if !matches!(tag, "INS" | "DEL" | "UPD" | "SEL") {
        return None;
    }

    // 先扫描 SQL 是否含占位符，大多数 SQL 没有占位符，可以提前返回，
    // 避免两次 CompactString 分配（trxid + statement key）。
    let (placeholder_count, detected_colon) = count_placeholders(pm_sql);
    if placeholder_count == 0 {
        return None;
    }

    let key = (
        CompactString::from(meta.trxid.as_ref()),
        CompactString::from(meta.statement.as_ref()),
    );

    // 消耗 buffer 条目：每个 PARAMS 只对应紧跟其后的一次执行
    let params = buffer.remove(&key)?;

    let colon_style = placeholder_override.unwrap_or(detected_colon);

    if params.len() != placeholder_count {
        log::warn!(
            "replace_parameters: param count mismatch (params={}, placeholders={}) for sql: {}",
            params.len(),
            placeholder_count,
            &pm_sql[..pm_sql.len().min(120)]
        );
        return None;
    }

    apply_params_into(pm_sql, &params, colon_style, scratch);

    // 常规路径：UTF-8 文件，直接返回。
    // GB18030 fallback：上游 parser 将 GB18030 文件按 UTF-8 解析时，param 替换后
    // 的字节序列可能含 GB18030 双字节序列（如汉字），导致 UTF-8 校验失败。
    // GB18030 是 ASCII 的超集，可安全处理纯 ASCII 与混合内容。
    if std::str::from_utf8(scratch).is_err() {
        let (decoded, _, had_errors) = encoding_rs::GB18030.decode(scratch);
        if had_errors {
            log::warn!(
                "replace_parameters: GB18030 fallback had unmappable bytes for sql: {}",
                &pm_sql[..pm_sql.len().min(60)]
            );
        }
        // into_owned() 释放对 scratch 的借用，之后才能 clear + 写回
        let decoded_string = decoded.into_owned();
        scratch.clear();
        scratch.extend_from_slice(decoded_string.as_bytes());
    }

    Some(std::str::from_utf8(scratch).expect("scratch contains valid UTF-8"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bare(s: &str) -> ParamValue {
        ParamValue::Bare(CompactString::new(s))
    }
    fn quoted(s: &str) -> ParamValue {
        ParamValue::Quoted(CompactString::new(s))
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
