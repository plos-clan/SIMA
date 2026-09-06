#![forbid(unsafe_code)]
#![forbid(clippy::unwrap_used)]

mod config;
mod ipc;
mod logger;
mod mount;
mod service;

use crate::config::SimaConfig;
use crate::logger::Log;
use crate::mount::MountService;
use crate::service::ServiceManager;
use figlet_rs::FIGlet;
use log::{error as fatal, info};
use platform_info::{PlatformInfo, PlatformInfoAPI, UNameAPI};

fn sysinfo_test() -> PlatformInfo {
    let pid = std::process::id();
    if pid != 1 {
        eprintln!("sima must be run in pid 1, now pid is {pid}.");
        std::process::exit(-1);
    }

    if let Ok(font) = FIGlet::standard()
        && let Some(banner) = font.convert("SIMA")
    {
        println!("{banner}");
    }
    PlatformInfo::new().expect("Unable to get platform info")
}

#[tokio::main]
async fn main() {
    Log::init_logger().unwrap_or_else(|e| {
        eprintln!("ERROR: Failed to initialize logger: {e}");
        std::process::exit(-1);
    });

    info!(
        "System Init & Management Agent v{}",
        env!("CARGO_PKG_VERSION")
    );
    let sys_info = sysinfo_test();
    info!("Machine info: {}", sys_info.machine().display());

    MountService::mount();

    let config = SimaConfig::load().unwrap_or_else(|e| {
        fatal!("ERROR: Failed to load config: {e}, Please check your /etc/sima.yml or /etc/sima.d/");
        std::process::exit(-1);
    });

    let mut manager = ServiceManager::new(config);
    if let Err(e) = manager.run().await {
        fatal!("Fatal: ServiceManager crashed: {}", e);
        std::process::exit(-1);
    }
}
