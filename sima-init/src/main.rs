#![forbid(unsafe_code)]
#![forbid(clippy::unwrap_used)]

mod config;
mod ipc;
mod logger;
mod service;

use crate::config::SimaConfig;
use crate::logger::Log;
use crate::service::ServiceManager;
use figlet_rs::FIGfont;
use platform_info::{PlatformInfo, PlatformInfoAPI, UNameAPI};
use spdlog::{error as fatal, info};
use std::path::PathBuf;

fn sysinfo_test() -> PlatformInfo {
    let pid = std::process::id();
    if pid != 1 {
        eprintln!("sima must be run in pid 1, now pid is {pid}.");
        std::process::exit(-1);
    }

    if let Ok(font) = FIGfont::standard()
        && let Some(banner) = font.convert("SIMA")
    {
        println!("{banner}");
    }
    PlatformInfo::new().expect("Unable to get platform info")
}

#[tokio::main]
async fn main() {
    let logdir = PathBuf::from("/var/log/sima");
    Log::init(Some(logdir), true).unwrap_or_else(|e| {
        eprintln!("ERROR: Failed to initialize logger: {e}");
        std::process::exit(-1);
    });

    info!(
        "System Init & Management Agent v{}",
        env!("CARGO_PKG_VERSION")
    );
    let sys_info = sysinfo_test();
    info!("Machine info: {}", sys_info.machine().display());

    let config = SimaConfig::load().unwrap_or_else(|e| {
        eprintln!("ERROR: Failed to load config: {e}");
        std::process::exit(-1);
    });

    let mut manager = ServiceManager::new(config);
    if let Err(e) = manager.run().await {
        fatal!("Fatal: ServiceManager crashed: {}", e);
        std::process::exit(-1);
    }
}
