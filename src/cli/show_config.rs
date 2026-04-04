use crate::color;
use crate::config::Config;

pub fn handle_show_config(cfg: &Config, config_path: &str) {
    let header = format!("Configuration ({config_path})");
    println!("{}", color::bold(&header));
    println!("{}", color::dim("═".repeat(header.len())));
    println!();

    // [sqllog]
    println!("{}", color::cyan("[sqllog]"));
    kv("directory", &cfg.sqllog.directory);
    println!();

    // [error]
    println!("{}", color::cyan("[error]"));
    kv("file", &cfg.error.file);
    println!();

    // [logging]
    println!("{}", color::cyan("[logging]"));
    kv("file", &cfg.logging.file);
    kv("level", &cfg.logging.level);
    kv("retention_days", &cfg.logging.retention_days.to_string());
    println!();

    // [exporter.*]
    if let Some(csv) = &cfg.exporter.csv {
        println!("{}", color::cyan("[exporter.csv]"));
        kv("file", &csv.file);
        kv("overwrite", &csv.overwrite.to_string());
        kv("append", &csv.append.to_string());
        println!();
    }

    if let Some(jsonl) = &cfg.exporter.jsonl {
        println!("{}", color::cyan("[exporter.jsonl]"));
        kv("file", &jsonl.file);
        kv("overwrite", &jsonl.overwrite.to_string());
        kv("append", &jsonl.append.to_string());
        println!();
    }

    if let Some(sqlite) = &cfg.exporter.sqlite {
        println!("{}", color::cyan("[exporter.sqlite]"));
        kv("database_url", &sqlite.database_url);
        kv("table_name", &sqlite.table_name);
        kv("overwrite", &sqlite.overwrite.to_string());
        kv("append", &sqlite.append.to_string());
        println!();
    }

    // [features]
    if let Some(rp) = &cfg.features.replace_parameters {
        println!("{}", color::cyan("[features.replace_parameters]"));
        kv("enable", &rp.enable.to_string());
        if !rp.placeholders.is_empty() {
            kv("placeholders", &format!("{:?}", rp.placeholders));
        }
        println!();
    }

    if let Some(f) = &cfg.features.filters {
        println!("{}", color::cyan("[features.filters]"));
        kv("enable", &f.enable.to_string());
        if let Some(s) = &f.meta.start_ts {
            kv("start_ts", s);
        }
        if let Some(e) = &f.meta.end_ts {
            kv("end_ts", e);
        }
        if let Some(ids) = &f.meta.trxids {
            kv("trxids", &format!("{} entries", ids.len()));
        }
        if let Some(users) = &f.meta.usernames {
            kv("usernames", &users.join(", "));
        }
        if let Some(ips) = &f.meta.client_ips {
            kv("client_ips", &ips.join(", "));
        }
        println!();
    }
}

fn kv(key: &str, value: &str) {
    println!("  {:<20} = {}", key, color::green(format!("\"{value}\"")));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, ExporterConfig, JsonlExporter, SqliteExporter};
    use crate::features::{FeaturesConfig, FiltersFeature, ReplaceParametersConfig};

    #[test]
    fn test_handle_show_config_default_does_not_panic() {
        let cfg = Config::default();
        // Just verify no panic — output goes to stdout which is fine in tests
        handle_show_config(&cfg, "config.toml");
    }

    #[test]
    fn test_handle_show_config_with_jsonl_exporter() {
        let cfg = Config {
            exporter: ExporterConfig {
                csv: None,
                jsonl: Some(JsonlExporter {
                    file: "out.jsonl".to_string(),
                    overwrite: true,
                    append: false,
                }),
                sqlite: None,
            },
            ..Default::default()
        };
        handle_show_config(&cfg, "test_config.toml");
    }

    #[test]
    fn test_handle_show_config_with_sqlite_exporter() {
        let cfg = Config {
            exporter: ExporterConfig {
                csv: None,
                jsonl: None,
                sqlite: Some(SqliteExporter {
                    database_url: "out.db".to_string(),
                    table_name: "logs".to_string(),
                    overwrite: false,
                    append: true,
                }),
            },
            ..Default::default()
        };
        handle_show_config(&cfg, "sqlite_config.toml");
    }

    #[test]
    fn test_handle_show_config_with_replace_parameters() {
        let cfg = Config {
            features: FeaturesConfig {
                replace_parameters: Some(ReplaceParametersConfig {
                    enable: true,
                    placeholders: vec!["?".to_string()],
                }),
                filters: None,
            },
            ..Default::default()
        };
        handle_show_config(&cfg, "rp_config.toml");
    }

    #[test]
    fn test_handle_show_config_with_filters() {
        let cfg = Config {
            features: FeaturesConfig {
                replace_parameters: None,
                filters: Some(FiltersFeature {
                    enable: true,
                    ..Default::default()
                }),
            },
            ..Default::default()
        };
        handle_show_config(&cfg, "filter_config.toml");
    }
}
