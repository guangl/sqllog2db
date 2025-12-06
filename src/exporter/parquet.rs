use crate::error::Result;
use crate::exporter::{ExportStats, util::f32_ms_to_i64};
use arrow::array::{ArrayRef, Int32Array, Int64Array, StringArray};
use arrow::datatypes::{DataType, Field, Schema};
use arrow::record_batch::RecordBatch;
use dm_database_parser_sqllog::Sqllog;
use log::info;
use parquet::arrow::ArrowWriter;
use parquet::file::properties::WriterProperties;
use rayon::prelude::*;
use std::fs::File;
use std::io::BufWriter;
use std::path::Path;
use std::sync::Arc;

/// Parquet 导出器 - 使用 Arrow 和 Parquet 生成真正的 Parquet 格式文件
pub struct ParquetExporter {
    pub file: String,
    pub overwrite: bool,
    pub row_group_size: usize,
    pub use_dictionary: bool,
    pub stats: ExportStats,
    pub schema: Arc<Schema>,
    pub writer: Option<ArrowWriter<BufWriter<File>>>,
    pub initialized: bool,
    // 缓存数据用于批量写入
    pub ts_vec: Vec<String>,
    pub ep_vec: Vec<i32>,
    pub sess_id_vec: Vec<String>,
    pub thrd_id_vec: Vec<String>,
    pub username_vec: Vec<String>,
    pub trx_id_vec: Vec<String>,
    pub statement_vec: Vec<String>,
    pub appname_vec: Vec<String>,
    pub client_ip_vec: Vec<String>,
    pub sql_vec: Vec<String>,
    pub exec_time_vec: Vec<i64>,
    pub row_count_vec: Vec<i64>,
    pub exec_id_vec: Vec<i64>,
}

impl std::fmt::Debug for ParquetExporter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ParquetExporter")
            .field("file", &self.file)
            .field("overwrite", &self.overwrite)
            .field("row_group_size", &self.row_group_size)
            .field("use_dictionary", &self.use_dictionary)
            .field("stats", &self.stats)
            .field("initialized", &self.initialized)
            .finish_non_exhaustive()
    }
}

impl ParquetExporter {
    #[must_use]
    pub fn new(file: String, overwrite: bool, row_group_size: usize, use_dictionary: bool) -> Self {
        // 内存优化：使用更小的行组大小
        // 原来: 3.5M 记录 = 2.37GB 峰值内存
        // 新的: 100k 记录 = ~70MB 峰值内存
        let actual_row_group_size = (row_group_size / 35).max(100_000);

        let schema = Arc::new(Schema::new(vec![
            Field::new("ts", DataType::Utf8, false),
            Field::new("ep", DataType::Int32, false),
            Field::new("sess_id", DataType::Utf8, false),
            Field::new("thrd_id", DataType::Utf8, false),
            Field::new("username", DataType::Utf8, false),
            Field::new("trx_id", DataType::Utf8, false),
            Field::new("statement", DataType::Utf8, false),
            Field::new("appname", DataType::Utf8, false),
            Field::new("client_ip", DataType::Utf8, false),
            Field::new("sql", DataType::Utf8, false),
            Field::new("exec_time_ms", DataType::Int64, false),
            Field::new("row_count", DataType::Int64, false),
            Field::new("exec_id", DataType::Int64, false),
        ]));

        Self {
            file,
            overwrite,
            row_group_size: actual_row_group_size,
            use_dictionary,
            stats: ExportStats::new(),
            schema,
            writer: None,
            initialized: false,
            ts_vec: Vec::with_capacity(actual_row_group_size),
            ep_vec: Vec::with_capacity(actual_row_group_size),
            sess_id_vec: Vec::with_capacity(actual_row_group_size),
            thrd_id_vec: Vec::with_capacity(actual_row_group_size),
            username_vec: Vec::with_capacity(actual_row_group_size),
            trx_id_vec: Vec::with_capacity(actual_row_group_size),
            statement_vec: Vec::with_capacity(actual_row_group_size),
            appname_vec: Vec::with_capacity(actual_row_group_size),
            client_ip_vec: Vec::with_capacity(actual_row_group_size),
            sql_vec: Vec::with_capacity(actual_row_group_size),
            exec_time_vec: Vec::with_capacity(actual_row_group_size),
            row_count_vec: Vec::with_capacity(actual_row_group_size),
            exec_id_vec: Vec::with_capacity(actual_row_group_size),
        }
    }

    #[must_use]
    pub fn from_config(config: &crate::config::ParquetExporter) -> Self {
        let row_group_size = config.row_group_size.unwrap_or(100_000);
        let use_dictionary = config.use_dictionary.unwrap_or(true);
        Self::new(
            config.file.clone(),
            config.overwrite,
            row_group_size,
            use_dictionary,
        )
    }

    pub fn initialize(&mut self) -> Result<()> {
        if self.initialized {
            return Ok(());
        }
        let path = Path::new(&self.file);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        if self.overwrite && path.exists() {
            std::fs::remove_file(path)?;
        }

        // 创建 Parquet Writer with BufWriter for better I/O performance
        let file = File::create(&self.file)?;
        let buf_writer = BufWriter::with_capacity(32 * 1024 * 1024, file); // 32MB buffer for faster writes
        let props_builder = WriterProperties::builder()
            .set_max_row_group_size(self.row_group_size)
            .set_compression(parquet::basic::Compression::UNCOMPRESSED)
            .set_dictionary_enabled(self.use_dictionary);

        let props = props_builder.build();
        let writer = ArrowWriter::try_new(buf_writer, self.schema.clone(), Some(props))
            .map_err(|e| std::io::Error::other(e.to_string()))?;

        self.writer = Some(writer);
        self.initialized = true;
        info!("ParquetExporter initialized: {}", self.file);
        Ok(())
    }

    pub fn export(&mut self, sqllog: &Sqllog<'_>) -> Result<()> {
        if !self.initialized {
            self.initialize()?;
        }

        let meta = sqllog.parse_meta();
        let ind = sqllog.parse_indicators();

        // 将数据添加到缓存
        self.ts_vec.push(sqllog.ts.to_string());
        self.ep_vec.push(i32::from(meta.ep));
        self.sess_id_vec.push(meta.sess_id.to_string());
        self.thrd_id_vec.push(meta.thrd_id.to_string());
        self.username_vec.push(meta.username.to_string());
        self.trx_id_vec.push(meta.trxid.to_string());
        self.statement_vec.push(meta.statement.to_string());
        self.appname_vec.push(meta.appname.to_string());
        self.client_ip_vec.push(meta.client_ip.to_string());
        self.sql_vec.push(sqllog.body().to_string());
        let exec_time = ind.as_ref().map_or(0, |i| f32_ms_to_i64(i.execute_time));
        self.exec_time_vec.push(exec_time);
        self.row_count_vec
            .push(ind.as_ref().map_or(0, |i| i64::from(i.row_count)));
        self.exec_id_vec
            .push(ind.as_ref().map_or(0, |i| i.execute_id));

        // 当缓存达到 row_group_size 时，写入一个 RecordBatch
        if self.ts_vec.len() >= self.row_group_size {
            self.flush()?;
        }

        self.stats.record_success();
        Ok(())
    }

    pub fn flush(&mut self) -> Result<()> {
        if self.ts_vec.is_empty() {
            return Ok(());
        }

        if let Some(writer) = &mut self.writer {
            // 直接使用 Vec 创建数组，无需 Arc::new 包装后再 take
            let batch = RecordBatch::try_new(
                self.schema.clone(),
                vec![
                    Arc::new(StringArray::from(std::mem::take(&mut self.ts_vec))) as ArrayRef,
                    Arc::new(Int32Array::from(std::mem::take(&mut self.ep_vec))) as ArrayRef,
                    Arc::new(StringArray::from(std::mem::take(&mut self.sess_id_vec))) as ArrayRef,
                    Arc::new(StringArray::from(std::mem::take(&mut self.thrd_id_vec))) as ArrayRef,
                    Arc::new(StringArray::from(std::mem::take(&mut self.username_vec))) as ArrayRef,
                    Arc::new(StringArray::from(std::mem::take(&mut self.trx_id_vec))) as ArrayRef,
                    Arc::new(StringArray::from(std::mem::take(&mut self.statement_vec)))
                        as ArrayRef,
                    Arc::new(StringArray::from(std::mem::take(&mut self.appname_vec))) as ArrayRef,
                    Arc::new(StringArray::from(std::mem::take(&mut self.client_ip_vec)))
                        as ArrayRef,
                    Arc::new(StringArray::from(std::mem::take(&mut self.sql_vec))) as ArrayRef,
                    Arc::new(Int64Array::from(std::mem::take(&mut self.exec_time_vec))) as ArrayRef,
                    Arc::new(Int64Array::from(std::mem::take(&mut self.row_count_vec))) as ArrayRef,
                    Arc::new(Int64Array::from(std::mem::take(&mut self.exec_id_vec))) as ArrayRef,
                ],
            )
            .map_err(|e| std::io::Error::other(e.to_string()))?;

            // 写入 RecordBatch
            writer
                .write(&batch)
                .map_err(|e| std::io::Error::other(e.to_string()))?;

            self.stats.flush_operations += 1;
            self.stats.last_flush_size = batch.num_rows();
        }

        Ok(())
    }

    pub fn finalize(&mut self) -> Result<()> {
        // 写入剩余数据
        self.flush()?;

        if let Some(writer) = self.writer.take() {
            writer
                .close()
                .map_err(|e| std::io::Error::other(e.to_string()))?;
        }

        info!(
            "Parquet export finished: {} (success: {}, failed: {})",
            self.file, self.stats.exported, self.stats.failed
        );
        self.initialized = false;
        Ok(())
    }

    #[must_use]
    pub fn name() -> &'static str {
        "Parquet"
    }

    #[must_use]
    pub fn stats_snapshot(&self) -> ExportStats {
        self.stats.clone()
    }
}

impl crate::exporter::Exporter for ParquetExporter {
    fn initialize(&mut self) -> Result<()> {
        self.initialize()
    }

    fn export(&mut self, sqllog: &Sqllog<'_>) -> Result<()> {
        self.export(sqllog)
    }

    fn finalize(&mut self) -> Result<()> {
        self.finalize()
    }

    fn name(&self) -> &str {
        Self::name()
    }

    fn stats_snapshot(&self) -> Option<ExportStats> {
        Some(self.stats_snapshot())
    }

    fn export_batch(&mut self, sqllogs: &[&Sqllog<'_>]) -> Result<()> {
        if !self.initialized {
            self.initialize()?;
        }

        // 并行提取所有字段
        let records: Vec<_> = sqllogs
            .par_iter()
            .map(|sqllog| {
                let meta = sqllog.parse_meta();
                let ind = sqllog.parse_indicators();
                (
                    sqllog.ts.to_string(),
                    i32::from(meta.ep),
                    meta.sess_id.to_string(),
                    meta.thrd_id.to_string(),
                    meta.username.to_string(),
                    meta.trxid.to_string(),
                    meta.statement.to_string(),
                    meta.appname.to_string(),
                    meta.client_ip.to_string(),
                    sqllog.body().to_string(),
                    ind.as_ref().map_or(0, |i| f32_ms_to_i64(i.execute_time)),
                    ind.as_ref().map_or(0, |i| i64::from(i.row_count)),
                    ind.as_ref().map_or(0, |i| i.execute_id),
                )
            })
            .collect();

        // 顺序添加到缓存向量
        for (
            ts,
            ep,
            sess_id,
            thrd_id,
            username,
            trx_id,
            statement,
            appname,
            client_ip,
            sql,
            exec_time,
            row_count,
            exec_id,
        ) in records
        {
            self.ts_vec.push(ts);
            self.ep_vec.push(ep);
            self.sess_id_vec.push(sess_id);
            self.thrd_id_vec.push(thrd_id);
            self.username_vec.push(username);
            self.trx_id_vec.push(trx_id);
            self.statement_vec.push(statement);
            self.appname_vec.push(appname);
            self.client_ip_vec.push(client_ip);
            self.sql_vec.push(sql);
            self.exec_time_vec.push(exec_time);
            self.row_count_vec.push(row_count);
            self.exec_id_vec.push(exec_id);
        }

        // 当缓存达到 row_group_size 时，写入一个 RecordBatch
        if self.ts_vec.len() >= self.row_group_size {
            self.flush()?;
        }

        self.stats.exported += sqllogs.len();
        Ok(())
    }
}

impl Drop for ParquetExporter {
    fn drop(&mut self) {
        if self.initialized
            && let Err(e) = self.finalize()
        {
            log::warn!("Parquet exporter finalization on Drop failed: {e}");
        }
    }
}
