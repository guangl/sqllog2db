use crate::error::Result;
use crate::exporter::ExportStats;
use arrow::array::{ArrayRef, Int32Array, Int64Array, StringArray};
use arrow::datatypes::{DataType, Field, Schema};
use arrow::record_batch::RecordBatch;
use dm_database_parser_sqllog::Sqllog;
use log::info;
use parquet::arrow::ArrowWriter;
use parquet::file::properties::WriterProperties;
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

impl ParquetExporter {
    pub fn new(file: String, overwrite: bool, row_group_size: usize, use_dictionary: bool) -> Self {
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
            row_group_size,
            use_dictionary,
            stats: ExportStats::new(),
            schema,
            writer: None,
            initialized: false,
            ts_vec: Vec::with_capacity(row_group_size),
            ep_vec: Vec::with_capacity(row_group_size),
            sess_id_vec: Vec::with_capacity(row_group_size),
            thrd_id_vec: Vec::with_capacity(row_group_size),
            username_vec: Vec::with_capacity(row_group_size),
            trx_id_vec: Vec::with_capacity(row_group_size),
            statement_vec: Vec::with_capacity(row_group_size),
            appname_vec: Vec::with_capacity(row_group_size),
            client_ip_vec: Vec::with_capacity(row_group_size),
            sql_vec: Vec::with_capacity(row_group_size),
            exec_time_vec: Vec::with_capacity(row_group_size),
            row_count_vec: Vec::with_capacity(row_group_size),
            exec_id_vec: Vec::with_capacity(row_group_size),
        }
    }

    pub fn from_config(config: &crate::config::ParquetExporter) -> Self {
        let row_group_size = config.row_group_size.unwrap_or(100000);
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
        let buf_writer = BufWriter::with_capacity(8 * 1024 * 1024, file); // 8MB buffer
        let mut props_builder =
            WriterProperties::builder().set_max_row_group_size(self.row_group_size);

        if self.use_dictionary {
            props_builder = props_builder.set_dictionary_enabled(true);
        }

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
        self.ep_vec.push(meta.ep as i32);
        self.sess_id_vec.push(meta.sess_id.to_string());
        self.thrd_id_vec.push(meta.thrd_id.to_string());
        self.username_vec.push(meta.username.to_string());
        self.trx_id_vec.push(meta.trxid.to_string());
        self.statement_vec.push(meta.statement.to_string());
        self.appname_vec.push(meta.appname.to_string());
        self.client_ip_vec.push(meta.client_ip.to_string());
        self.sql_vec.push(sqllog.body().to_string());
        self.exec_time_vec
            .push(ind.as_ref().map_or(0, |i| i.execute_time as i64));
        self.row_count_vec
            .push(ind.as_ref().map_or(0, |i| i.row_count as i64));
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
            // 使用 std::mem::take 避免克隆,直接转移所有权
            let ts_array: ArrayRef = Arc::new(StringArray::from(std::mem::take(&mut self.ts_vec)));
            let ep_array: ArrayRef = Arc::new(Int32Array::from(std::mem::take(&mut self.ep_vec)));
            let sess_id_array: ArrayRef =
                Arc::new(StringArray::from(std::mem::take(&mut self.sess_id_vec)));
            let thrd_id_array: ArrayRef =
                Arc::new(StringArray::from(std::mem::take(&mut self.thrd_id_vec)));
            let username_array: ArrayRef =
                Arc::new(StringArray::from(std::mem::take(&mut self.username_vec)));
            let trx_id_array: ArrayRef =
                Arc::new(StringArray::from(std::mem::take(&mut self.trx_id_vec)));
            let statement_array: ArrayRef =
                Arc::new(StringArray::from(std::mem::take(&mut self.statement_vec)));
            let appname_array: ArrayRef =
                Arc::new(StringArray::from(std::mem::take(&mut self.appname_vec)));
            let client_ip_array: ArrayRef =
                Arc::new(StringArray::from(std::mem::take(&mut self.client_ip_vec)));
            let sql_array: ArrayRef =
                Arc::new(StringArray::from(std::mem::take(&mut self.sql_vec)));
            let exec_time_array: ArrayRef =
                Arc::new(Int64Array::from(std::mem::take(&mut self.exec_time_vec)));
            let row_count_array: ArrayRef =
                Arc::new(Int64Array::from(std::mem::take(&mut self.row_count_vec)));
            let exec_id_array: ArrayRef =
                Arc::new(Int64Array::from(std::mem::take(&mut self.exec_id_vec)));

            // 创建 RecordBatch
            let batch = RecordBatch::try_new(
                self.schema.clone(),
                vec![
                    ts_array,
                    ep_array,
                    sess_id_array,
                    thrd_id_array,
                    username_array,
                    trx_id_array,
                    statement_array,
                    appname_array,
                    client_ip_array,
                    sql_array,
                    exec_time_array,
                    row_count_array,
                    exec_id_array,
                ],
            )
            .map_err(|e| std::io::Error::other(e.to_string()))?;

            // 写入 RecordBatch
            writer
                .write(&batch)
                .map_err(|e| std::io::Error::other(e.to_string()))?;

            // std::mem::take 已经清空了所有 Vec,无需再次调用 clear()

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

    pub fn name(&self) -> &str {
        "Parquet"
    }

    pub fn stats_snapshot(&self) -> Option<ExportStats> {
        Some(self.stats.clone())
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
        self.name()
    }

    fn stats_snapshot(&self) -> Option<ExportStats> {
        self.stats_snapshot()
    }
}

impl Drop for ParquetExporter {
    fn drop(&mut self) {
        if self.initialized {
            if let Err(e) = self.finalize() {
                log::warn!("Parquet exporter finalization on Drop failed: {}", e);
            }
        }
    }
}
