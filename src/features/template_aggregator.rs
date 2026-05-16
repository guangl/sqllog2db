use hdrhistogram::Histogram;

/// 单个 SQL 模板的内部统计条目（私有）
#[derive(Debug)]
struct TemplateEntry {
    histogram: Histogram<u64>,
    first_seen: String,
    last_seen: String,
}

impl TemplateEntry {
    fn new(first_seen: String) -> Self {
        let histogram = Histogram::<u64>::new_with_bounds(1, 60_000_000, 2)
            .expect("TemplateEntry: invalid histogram bounds");
        let last_seen = first_seen.clone();
        Self {
            histogram,
            first_seen,
            last_seen,
        }
    }
}

/// 单个 SQL 模板的聚合统计结果
#[derive(Debug, Clone, serde::Serialize)]
pub struct TemplateStats {
    pub template_key: String,
    pub count: u64,
    pub avg_us: u64,
    pub min_us: u64,
    pub max_us: u64,
    pub p50_us: u64,
    pub p95_us: u64,
    pub p99_us: u64,
    pub first_seen: String,
    pub last_seen: String,
}

/// 图表生成专用的只读视图：在 finalize 前调用，不消耗 self
#[derive(Debug)]
#[allow(dead_code)] // Phase 15 Plan 03+ 将实现图表生成时使用
pub struct ChartEntry<'a> {
    pub key: &'a str,
    pub count: u64,
    pub histogram: &'a hdrhistogram::Histogram<u64>,
}

/// SQL 模板执行时间聚合器
///
/// 每个模板 key（来自 `normalize_template()`）对应一个 hdrhistogram，
/// 存储耗时样本（微秒）。支持 `observe()` 热循环累积、`merge()` 并行合并、
/// `finalize()` 输出统计结果。
#[derive(Debug, Default)]
pub struct TemplateAggregator {
    entries: ahash::AHashMap<String, TemplateEntry>,
}

impl TemplateAggregator {
    /// 创建新的聚合器
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// 记录一次 SQL 模板执行观测
    ///
    /// - `key`: 已归一化的模板 key（来自 `normalize_template()`）
    /// - `exectime_us`: 执行时间（微秒）
    /// - `ts`: 时间戳字符串（达梦日志 ISO 8601 格式，字典序与时间序一致）
    pub fn observe(&mut self, key: &str, exectime_us: u64, ts: &str) {
        let entry = self
            .entries
            .entry(key.to_string())
            .or_insert_with(|| TemplateEntry::new(ts.to_string()));

        // 箝位到 [1, 60_000_000]：0us（< 1ms 的缓存命中查询）和超长慢查询都能计入（WR-01）
        let clamped = exectime_us.clamp(1, 60_000_000);
        let _ = entry.histogram.record(clamped);

        if ts < entry.first_seen.as_str() {
            entry.first_seen = ts.to_string();
        }
        if ts > entry.last_seen.as_str() {
            entry.last_seen = ts.to_string();
        }
    }

    /// 合并另一个聚合器的结果（用于 rayon map-reduce 并行路径）
    ///
    /// # Panics
    ///
    /// 如果两个聚合器中的 histogram 边界不一致（bounds mismatch），则 panic。
    /// 正常情况下所有 `TemplateEntry` 都使用相同的边界（`new_with_bounds(1, 60_000_000, 2)`），
    /// 该 panic 只在代码逻辑错误时触发。
    pub fn merge(&mut self, other: TemplateAggregator) {
        for (key, other_entry) in other.entries {
            match self.entries.get_mut(&key) {
                Some(entry) => {
                    entry
                        .histogram
                        .add(&other_entry.histogram)
                        .expect("histogram bounds mismatch: all TemplateEntry histograms must use identical bounds");

                    if other_entry.first_seen < entry.first_seen {
                        entry.first_seen = other_entry.first_seen;
                    }
                    if other_entry.last_seen > entry.last_seen {
                        entry.last_seen = other_entry.last_seen;
                    }
                }
                None => {
                    self.entries.insert(key, other_entry);
                }
            }
        }
    }

    /// 将聚合结果转换为统计列表，按 count 降序排列
    #[must_use]
    pub fn finalize(self) -> Vec<TemplateStats> {
        let mut stats: Vec<TemplateStats> = self
            .entries
            .into_iter()
            .map(|(key, entry)| {
                let h = &entry.histogram;
                TemplateStats {
                    template_key: key,
                    count: h.len(),
                    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                    avg_us: h.mean() as u64,
                    min_us: h.min(),
                    max_us: h.max(),
                    p50_us: h.value_at_quantile(0.50),
                    p95_us: h.value_at_quantile(0.95),
                    p99_us: h.value_at_quantile(0.99),
                    first_seen: entry.first_seen,
                    last_seen: entry.last_seen,
                }
            })
            .collect();

        stats.sort_unstable_by(|a, b| {
            b.count
                .cmp(&a.count)
                .then_with(|| a.template_key.cmp(&b.template_key))
        });
        stats
    }

    /// 返回图表生成专用的只读迭代器，按 count 降序排列（count 相同时按 key 升序）
    ///
    /// 在 `finalize()` 之前调用，不消耗 self。
    #[allow(dead_code)] // Phase 15 Plan 03+ 将实现图表生成时使用
    pub fn iter_chart_entries(&self) -> impl Iterator<Item = ChartEntry<'_>> {
        let mut entries: Vec<ChartEntry<'_>> = self
            .entries
            .iter()
            .map(|(k, entry)| ChartEntry {
                key: k.as_str(),
                count: entry.histogram.len(),
                histogram: &entry.histogram,
            })
            .collect();
        entries.sort_unstable_by(|a, b| b.count.cmp(&a.count).then_with(|| a.key.cmp(b.key)));
        entries.into_iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_observe_single() {
        let mut agg = TemplateAggregator::new();
        agg.observe("SELECT * FROM t WHERE id = ?", 500, "2025-01-15 10:00:00");
        let stats = agg.finalize();
        assert_eq!(stats.len(), 1);
        assert_eq!(stats[0].count, 1);
        assert_eq!(stats[0].template_key, "SELECT * FROM t WHERE id = ?");
    }

    #[test]
    fn test_finalize_percentiles() {
        let mut agg = TemplateAggregator::new();
        let key = "SELECT * FROM t";
        // 插入 100 个样本：1..=100 微秒
        for i in 1u64..=100 {
            agg.observe(key, i, "2025-01-15 10:00:00");
        }
        let stats = agg.finalize();
        assert_eq!(stats.len(), 1);
        let s = &stats[0];
        assert_eq!(s.count, 100);
        // p50 应接近 50，允许 hdrhistogram sigfig=2 的误差（±2%）
        assert!(s.p50_us >= 48 && s.p50_us <= 52, "p50_us={}", s.p50_us);
        // p99 应接近 99
        assert!(s.p99_us >= 97 && s.p99_us <= 100, "p99_us={}", s.p99_us);
        // min/max
        assert_eq!(s.min_us, 1);
        assert_eq!(s.max_us, 100);
    }

    #[test]
    fn test_merge_equivalent() {
        let key = "SELECT 1";
        let ts = "2025-01-15 10:00:00";

        let mut agg1 = TemplateAggregator::new();
        agg1.observe(key, 100, ts);
        agg1.observe(key, 200, ts);

        let mut agg2 = TemplateAggregator::new();
        agg2.observe(key, 300, ts);
        agg2.observe(key, 400, ts);

        agg1.merge(agg2);
        let stats = agg1.finalize();
        assert_eq!(stats.len(), 1);
        assert_eq!(stats[0].count, 4);
        // min/max 允许 hdrhistogram sigfig=2 的量化误差（±1%）
        assert!(
            stats[0].min_us >= 99 && stats[0].min_us <= 101,
            "min_us={}",
            stats[0].min_us
        );
        assert!(
            stats[0].max_us >= 396 && stats[0].max_us <= 404,
            "max_us={}",
            stats[0].max_us
        );
    }

    #[test]
    fn test_merge_timestamps() {
        let key = "SELECT 1";

        let mut agg1 = TemplateAggregator::new();
        agg1.observe(key, 100, "2025-01-15 10:00:00");
        agg1.observe(key, 200, "2025-01-15 12:00:00");

        let mut agg2 = TemplateAggregator::new();
        agg2.observe(key, 300, "2025-01-15 08:00:00"); // 更早
        agg2.observe(key, 400, "2025-01-15 14:00:00"); // 更晚

        agg1.merge(agg2);
        let stats = agg1.finalize();
        assert_eq!(stats.len(), 1);
        // first_seen 应为最小值
        assert_eq!(stats[0].first_seen, "2025-01-15 08:00:00");
        // last_seen 应为最大值
        assert_eq!(stats[0].last_seen, "2025-01-15 14:00:00");
    }

    #[test]
    fn test_observe_first_last_seen() {
        let key = "SELECT 1";
        let mut agg = TemplateAggregator::new();
        agg.observe(key, 100, "2025-01-15 10:00:00");
        agg.observe(key, 200, "2025-01-14 09:00:00"); // 更早
        agg.observe(key, 300, "2025-01-16 11:00:00"); // 更晚
        let stats = agg.finalize();
        assert_eq!(stats[0].first_seen, "2025-01-14 09:00:00");
        assert_eq!(stats[0].last_seen, "2025-01-16 11:00:00");
    }

    #[test]
    fn test_finalize_sorts_by_count_desc() {
        let mut agg = TemplateAggregator::new();
        let ts = "2025-01-15 10:00:00";

        // key_a 2次，key_b 5次，key_c 1次 → 排序应为 b(5), a(2), c(1)
        agg.observe("key_a", 100, ts);
        agg.observe("key_a", 200, ts);

        for _ in 0..5 {
            agg.observe("key_b", 300, ts);
        }

        agg.observe("key_c", 400, ts);

        let stats = agg.finalize();
        assert_eq!(stats.len(), 3);
        assert_eq!(stats[0].template_key, "key_b");
        assert_eq!(stats[0].count, 5);
        assert_eq!(stats[1].template_key, "key_a");
        assert_eq!(stats[1].count, 2);
        assert_eq!(stats[2].template_key, "key_c");
        assert_eq!(stats[2].count, 1);
    }

    #[test]
    fn test_iter_chart_entries_empty() {
        let agg = TemplateAggregator::new();
        let entries: Vec<_> = agg.iter_chart_entries().collect();
        assert!(entries.is_empty());
    }

    #[test]
    fn test_iter_chart_entries_single_key() {
        let mut agg = TemplateAggregator::new();
        let key = "SELECT * FROM t WHERE id = ?";
        let ts = "2025-01-15 10:00:00";
        agg.observe(key, 100, ts);
        agg.observe(key, 200, ts);
        agg.observe(key, 300, ts);

        let entries: Vec<_> = agg.iter_chart_entries().collect();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].key, key);
        assert_eq!(entries[0].count, 3);
        // histogram 引用可调用 .len() 返回观测数量
        assert_eq!(entries[0].histogram.len(), 3);
    }

    #[test]
    fn test_iter_chart_entries_sort_order() {
        let mut agg = TemplateAggregator::new();
        let ts = "2025-01-15 10:00:00";

        // key_a 2次，key_b 5次，key_c 1次 → 迭代结果应为 [key_b(5), key_a(2), key_c(1)]
        agg.observe("key_a", 100, ts);
        agg.observe("key_a", 200, ts);

        for _ in 0..5 {
            agg.observe("key_b", 300, ts);
        }

        agg.observe("key_c", 400, ts);

        let entries: Vec<_> = agg.iter_chart_entries().collect();
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].key, "key_b");
        assert_eq!(entries[0].count, 5);
        assert_eq!(entries[1].key, "key_a");
        assert_eq!(entries[1].count, 2);
        assert_eq!(entries[2].key, "key_c");
        assert_eq!(entries[2].count, 1);
    }
}
