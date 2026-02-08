use std::path::PathBuf;
use std::sync::Arc;
use spdlog::{Level, LevelFilter, Logger, LoggerBuilder};
use spdlog::sink::{RotatingFileSink, RotationPolicy};
use crate::error::{SimaResult};

pub struct Log;

impl Log {
    pub fn new(logdir: Option<PathBuf>, verbose: bool) -> SimaResult<()> {
        let mut logger: LoggerBuilder = Logger::builder();
        logger.sinks(spdlog::default_logger().sinks().to_owned());

        let level = if verbose {
            LevelFilter::MoreSevereEqual(Level::Debug)
        } else {
            LevelFilter::MoreSevereEqual(Level::Info)
        };
        logger.level_filter(level);

        if let Some(logdir) = &logdir {
            let logdir = PathBuf::from(logdir);

            if !logdir.exists() && !logdir.is_dir() {
                std::fs::create_dir_all(logdir.clone())?
            }

            let log_name = format!("{}.log", env!("CARGO_PKG_NAME"));
            let logdir = PathBuf::from(logdir).join(log_name);

            let file_sink: Arc<RotatingFileSink> = Arc::new(
                RotatingFileSink::builder()
                    .base_path(logdir)
                    .rotation_policy(RotationPolicy::Daily { hour: 0, minute: 0 })
                    .rotate_on_open(false)
                    .build()
                    .unwrap(),
            );

            logger.sink(file_sink);
        }

        let logger = Arc::new(logger.build().unwrap());
        let _ = spdlog::swap_default_logger(logger);

        Ok(())
    }
}
