use anyhow::Result;
use std::io::Write;

pub struct Log;

impl Log {
    pub fn init_logger() -> Result<(), log::SetLoggerError> {
        env_logger::Builder::new()
            .filter_level(log::LevelFilter::Info)
            .format(|buf, record| writeln!(buf, "[{}] {}", record.level(), record.args()))
            .try_init()
    }
}
