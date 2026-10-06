// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: MPL-2.0

use std::io::Write as _;

use log::{LevelFilter, Log, Metadata, Record};

use crate::config::{LogConfig, LogLevel};

struct TextLogger;

impl Log for TextLogger {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        metadata.level() <= log::max_level()
    }

    fn log(&self, record: &Record<'_>) {
        if self.enabled(record.metadata()) {
            let _ = writeln!(
                std::io::stdout().lock(),
                "[{:<5}] {}: {}",
                record.level(),
                record.target(),
                record.args()
            );
        }
    }

    fn flush(&self) {
        let _ = std::io::stdout().flush();
    }
}

static LOGGER: TextLogger = TextLogger;

pub fn init(config: &LogConfig) {
    let default_level = if config.enabled {
        match config.level {
            LogLevel::Trace => LevelFilter::Trace,
            LogLevel::Debug => LevelFilter::Debug,
            LogLevel::Info => LevelFilter::Info,
            LogLevel::Warning => LevelFilter::Warn,
            LogLevel::Error => LevelFilter::Error,
        }
    } else {
        LevelFilter::Off
    };
    let level = std::env::var("PKG_EARTH_LOG")
        .map(|value| {
            value
                .parse()
                .unwrap_or_else(|_| panic!("invalid PKG_EARTH_LOG level: {value}"))
        })
        .unwrap_or(default_level);

    log::set_logger(&LOGGER).expect("text logger is initialized only once");
    log::set_max_level(level);
}
