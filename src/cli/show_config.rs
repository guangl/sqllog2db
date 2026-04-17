use crate::color;
use crate::config::Config;

pub fn handle_show_config(cfg: &Config, config_path: &str, diff: bool) {
    let def = if diff { Some(Config::default()) } else { None };

    let header = format!("Configuration ({config_path})");
    println!("{}", color::bold(&header));
    println!("{}", color::dim("═".repeat(header.len())));
    if diff {
        println!(
            "{}",
            color::dim("  Fields marked with * differ from defaults")
        );
    }
    println!();

    // [sqllog]
    println!("{}", color::cyan("[sqllog]"));
    kv(
        "path",
        &cfg.sqllog.path,
        def.as_ref().map(|d| d.sqllog.path.as_str()),
        diff,
    );
    println!();

    // [logging]
    println!("{}", color::cyan("[logging]"));
    kv(
        "file",
        &cfg.logging.file,
        def.as_ref().map(|d| d.logging.file.as_str()),
        diff,
    );
    kv(
        "level",
        &cfg.logging.level,
        def.as_ref().map(|d| d.logging.level.as_str()),
        diff,
    );
    let def_days = def.as_ref().map(|d| d.logging.retention_days.to_string());
    kv(
        "retention_days",
        &cfg.logging.retention_days.to_string(),
        def_days.as_deref(),
        diff,
    );
    println!();

    // [exporter.*]
    if let Some(csv) = &cfg.exporter.csv {
        let def_csv = def.as_ref().and_then(|d| d.exporter.csv.as_ref());
        println!("{}", color::cyan("[exporter.csv]"));
        kv("file", &csv.file, def_csv.map(|d| d.file.as_str()), diff);
        let def_ow = def_csv.map(|d| if d.overwrite { "true" } else { "false" });
        let def_ap = def_csv.map(|d| if d.append { "true" } else { "false" });
        kv("overwrite", &csv.overwrite.to_string(), def_ow, diff);
        kv("append", &csv.append.to_string(), def_ap, diff);
        println!();
    }

    if let Some(sqlite) = &cfg.exporter.sqlite {
        // sqlite is not in defaults, so all fields are "new" when diff=true
        let def_sqlite = def.as_ref().and_then(|d| d.exporter.sqlite.as_ref());
        println!("{}", color::cyan("[exporter.sqlite]"));
        kv(
            "database_url",
            &sqlite.database_url,
            def_sqlite.map(|d| d.database_url.as_str()),
            diff,
        );
        kv(
            "table_name",
            &sqlite.table_name,
            def_sqlite.map(|d| d.table_name.as_str()),
            diff,
        );
        let def_ow = def_sqlite.map(|d| if d.overwrite { "true" } else { "false" });
        let def_ap = def_sqlite.map(|d| if d.append { "true" } else { "false" });
        kv("overwrite", &sqlite.overwrite.to_string(), def_ow, diff);
        kv("append", &sqlite.append.to_string(), def_ap, diff);
        println!();
    }

    // [features]
    if let Some(rp) = &cfg.features.replace_parameters {
        println!("{}", color::cyan("[features.replace_parameters]"));
        kv("enable", &rp.enable.to_string(), None, diff);
        if !rp.placeholders.is_empty() {
            kv(
                "placeholders",
                &format!("{:?}", rp.placeholders),
                None,
                diff,
            );
        }
        println!();
    }

    if let Some(f) = &cfg.features.filters {
        println!("{}", color::cyan("[features.filters]"));
        kv("enable", &f.enable.to_string(), None, diff);
        if let Some(s) = &f.meta.start_ts {
            kv("start_ts", s, None, diff);
        }
        if let Some(e) = &f.meta.end_ts {
            kv("end_ts", e, None, diff);
        }
        if let Some(ids) = &f.meta.trxids {
            kv("trxids", &format!("{} entries", ids.len()), None, diff);
        }
        if let Some(users) = &f.meta.usernames {
            kv("usernames", &users.join(", "), None, diff);
        }
        if let Some(ips) = &f.meta.client_ips {
            kv("client_ips", &ips.join(", "), None, diff);
        }
        println!();
    }
}

/// Print a key=value line, optionally highlighting if the value differs from its default.
fn kv(key: &str, value: &str, default: Option<&str>, diff: bool) {
    let changed = diff && (default != Some(value));
    if changed {
        println!(
            "  {:<20} = {} {}",
            key,
            color::yellow(format!("\"{value}\"")),
            color::dim("*"),
        );
    } else {
        println!("  {:<20} = {}", key, color::green(format!("\"{value}\"")));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, ExporterConfig, SqliteExporter};
    use crate::features::{FeaturesConfig, FiltersFeature, ReplaceParametersConfig};

    #[test]
    fn test_handle_show_config_default_does_not_panic() {
        let cfg = Config::default();
        handle_show_config(&cfg, "config.toml", false);
    }

    #[test]
    fn test_handle_show_config_diff_no_changes() {
        let cfg = Config::default();
        // diff with no changes should not panic
        handle_show_config(&cfg, "config.toml", true);
    }

    #[test]
    fn test_handle_show_config_diff_with_changes() {
        let mut cfg = Config::default();
        cfg.sqllog.path = "/custom/logs".to_string();
        cfg.logging.level = "debug".to_string();
        handle_show_config(&cfg, "config.toml", true);
    }

    #[test]
    fn test_handle_show_config_with_sqlite_exporter() {
        let cfg = Config {
            exporter: ExporterConfig {
                csv: None,
                sqlite: Some(SqliteExporter {
                    database_url: "out.db".to_string(),
                    table_name: "logs".to_string(),
                    overwrite: false,
                    append: true,
                }),
            },
            ..Default::default()
        };
        handle_show_config(&cfg, "sqlite_config.toml", false);
    }

    #[test]
    fn test_handle_show_config_with_sqlite_diff() {
        let cfg = Config {
            exporter: ExporterConfig {
                csv: None,
                sqlite: Some(SqliteExporter::default()),
            },
            ..Default::default()
        };
        // sqlite is not in defaults, so all fields should be marked
        handle_show_config(&cfg, "sqlite_config.toml", true);
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
                fields: None,
            },
            ..Default::default()
        };
        handle_show_config(&cfg, "rp_config.toml", false);
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
                fields: None,
            },
            ..Default::default()
        };
        handle_show_config(&cfg, "filter_config.toml", false);
    }
}
