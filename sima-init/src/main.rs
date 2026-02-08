#![forbid(unsafe_code)]
#![forbid(clippy::unwrap_used)]

mod config;
pub mod error;
mod logger;
mod service;

use crate::config::SimaConfig;
use crate::logger::Log;
use crate::service::ServiceManager;
use platform_info::{PlatformInfo, PlatformInfoAPI, UNameAPI};
use std::path::PathBuf;
use spdlog::info;

fn sysinfo_test() -> PlatformInfo {
    let pid = std::process::id();
    if pid != 1 {
        eprintln!("sima must be run in pid 1, now pid is {pid}.");
        std::process::exit(-1);
    }

    println!(
        r#"
     ________       ___      _____ ______       ________
    |\   ____\     |\  \    |\   _ \  _   \    |\   __  \
    \ \  \___|_    \ \  \   \ \  \\\__\ \  \   \ \  \|\  \
     \ \_____  \    \ \  \   \ \  \\|__| \  \   \ \   __  \
      \|____|\  \    \ \  \   \ \  \    \ \  \   \ \  \ \  \
        ____\_\  \    \ \__\   \ \__\    \ \__\   \ \__\ \__\
       |\_________\    \|__|    \|__|     \|__|    \|__|\|__|
       \|_________|
    "#
    );
    PlatformInfo::new().expect("Unable to get platform info")
}

fn main() {
    let sys_info = sysinfo_test();
    let config = SimaConfig::load().unwrap_or_else(|e| {
        eprintln!("ERROR: Failed to load config: {e}");
        std::process::exit(-1);
    });
    let logdir = PathBuf::from("/var/log/sima");
    Log::new(Some(logdir), true).unwrap_or_else(|e| {
        eprintln!("ERROR: Failed to initialize logger: {e}");
        std::process::exit(-1);
    });
    info!("System Init & Management Agent v{}", env!("CARGO_PKG_VERSION"));
    info!("Machine info: {}", sys_info.machine().display());
    ServiceManager::new().run(config);
}
