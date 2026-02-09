use crate::config::{ServiceConfig, SimaConfig};
use anyhow::Result;
use nix::sys::signal::{self, Signal};
use nix::sys::wait::{WaitPidFlag, WaitStatus, waitpid};
use nix::unistd::Pid;
use spdlog::{error, info, warn};
use std::collections::HashMap;
use std::process::Command;
use std::time::Duration;
use tokio::signal::unix::Signal as TokioSignal;
use tokio::signal::unix::{SignalKind, signal as tokio_signal};
use tokio::time::timeout;

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum ServiceStatus {
    Running,
    Stopped,
    Errored,
}

pub struct ServiceState {
    pid: Option<Pid>,
    status: ServiceStatus,
}

impl Default for ServiceState {
    fn default() -> Self {
        Self {
            pid: None,
            status: ServiceStatus::Stopped,
        }
    }
}

pub struct ServiceManager {
    configs: HashMap<String, ServiceConfig>,
    states: HashMap<String, ServiceState>,
    pid_map: HashMap<Pid, String>,
}

impl ServiceManager {
    pub fn new(config: SimaConfig) -> Self {
        let mut configs = HashMap::new();
        let mut states = HashMap::new();

        for sc in config.services {
            states.insert(sc.name.clone(), ServiceState::default());
            configs.insert(sc.name.clone(), sc);
        }

        Self {
            configs,
            states,
            pid_map: HashMap::new(),
        }
    }

    fn spawn_process(cmdline: &str) -> Result<Pid> {
        let child = Command::new("/bin/sh")
            .arg("-c")
            .arg(format!("exec {}", cmdline))
            .process_group(0)
            .spawn()?;

        Ok(Pid::from_raw(child.id() as i32))
    }

    pub async fn run(&mut self) -> Result<()> {
        for (name, config) in &self.configs {
            if let Some(state) = self.states.get_mut(name) {
                Self::start_service(name, config, state, &mut self.pid_map);
            }
        }
        self.event_loop().await
    }

    fn start_service(
        name: &str,
        config: &ServiceConfig,
        state: &mut ServiceState,
        pid_map: &mut HashMap<Pid, String>,
    ) {
        if state.status == ServiceStatus::Running {
            return;
        }

        info!("Starting service: {}", name);
        match Self::spawn_process(&config.cmdline) {
            Ok(pid) => {
                info!("Service {} started (PID: {})", name, pid);
                state.pid = Some(pid);
                state.status = ServiceStatus::Running;
                pid_map.insert(pid, name.to_string());
            }
            Err(e) => {
                error!("Failed to start service {}: {}", name, e);
                state.status = ServiceStatus::Errored;
            }
        }
    }

    fn reap_zombies(&mut self) {
        loop {
            match waitpid(Pid::from_raw(-1), Some(WaitPidFlag::WNOHANG)) {
                Ok(WaitStatus::StillAlive) => break,
                Ok(status) => {
                    self.handle_process_exit(status);
                }
                Err(nix::Error::ECHILD) => break,
                Err(e) => {
                    error!("System error during waitpid: {}", e);
                    break;
                }
            }
        }
    }

    fn handle_process_exit(&mut self, status: WaitStatus) {
        let Some(pid) = status.pid() else {
            return;
        };

        if let Some(name) = self.pid_map.remove(&pid) {
            info!("Service {} (PID {}) {:?}", name, pid, status);

            if let Some(state) = self.states.get_mut(&name) {
                state.pid = None;
                state.status = ServiceStatus::Stopped;
            }
        } else {
            info!("Reaped orphan process PID {} ({:?})", pid, status);
        }
    }

    async fn event_loop(&mut self) -> Result<()> {
        let mut sigchld = tokio_signal(SignalKind::child())?;
        let mut sigterm = tokio_signal(SignalKind::terminate())?;
        let mut sigint = tokio_signal(SignalKind::interrupt())?;

        info!("Sima event loop started.");

        loop {
            tokio::select! {
                _ = sigchld.recv() => {
                    self.reap_zombies();
                },
                Some(_) = sigterm.recv() => {
                    info!("Received SIGTERM, shutting down...");
                    self.perform_shutdown(&mut sigchld).await;
                    break;
                }
                Some(_) = sigint.recv() => {
                    info!("Received SIGINT, shutting down...");
                    self.perform_shutdown(&mut sigchld).await;
                    break;
                }
            }
        }
        Ok(())
    }

    async fn perform_shutdown(&mut self, sigchld: &mut TokioSignal) {
        self.broadcast_signal(Signal::SIGTERM);

        let shutdown_timeout = Duration::from_secs(10);
        info!("Waiting for services to stop...");

        let wait_result = timeout(shutdown_timeout, async {
            loop {
                if self.pid_map.is_empty() {
                    return;
                }
                sigchld.recv().await;
                self.reap_zombies();
            }
        })
        .await;

        if wait_result.is_err() {
            warn!("Shutdown timed out, forcing kill...");
            self.broadcast_signal(Signal::SIGKILL);
            self.reap_zombies();
        } else {
            info!("All services stopped gracefully.");
        }
    }

    fn broadcast_signal(&self, sig: Signal) {
        for (pid, name) in &self.pid_map {
            let pgid = Pid::from_raw(-pid.as_raw());
            info!("Sending {} to {} (PID {})", sig, name, pid);
            if let Err(e) = signal::kill(pgid, sig)
                && e != nix::Error::ESRCH
            {
                warn!("Failed to send signal to {}: {}", name, e);
            }
        }
    }
}
