use crate::config::{ServiceConfig, SimaConfig};
use std::process::Command;
use spdlog::info;

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum ServiceStatus {
    Running,
    Stopped,
    Errored,
}

pub struct Service {
    pid: Option<u32>,
    status: ServiceStatus,
    config: ServiceConfig,
}

pub struct ServiceManager {
    services: Vec<Service>,
}

impl ServiceManager {
    pub fn new() -> Self {
        Self {
            services: Vec::new(),
        }
    }

    pub fn run(&mut self, config: SimaConfig) {
        info!("Launching services...");

        for service_config in config.services {
            self.launch_service(service_config);
        }
    }

    fn spawn_process(cmdline: &str) -> Result<u32, String> {
        let args = shell_words::split(cmdline)
            .map_err(|e| format!("Config syntax error: {}", e))?;

        let (program, args) = args.split_first()
            .ok_or_else(|| "Empty command line".to_string())?;

        let child = Command::new(program)
            .args(args)
            .spawn()
            .map_err(|e| format!("Failed to execute: {}", e))?;

        Ok(child.id())
    }

    fn launch_service(&mut self, config: ServiceConfig) {
        info!("Processing service: {}", config.name);

        let (status, pid) = match Self::spawn_process(&config.cmdline) {
            Ok(pid) => {
                info!("Service {} started (PID: {})", config.name, pid);
                (ServiceStatus::Running, Some(pid))
            }
            Err(err_msg) => {
                info!("Service {} error: {}", config.name, err_msg);
                (ServiceStatus::Errored, None)
            }
        };

        self.services.push(Service {
            pid,
            status,
            config,
        });
    }
}
