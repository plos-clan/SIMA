use crate::config::{ServiceConfig, SimaConfig};
use crate::ipc::{IpcCommand, IpcServer, handle_client};
use anyhow::Result;
use nix::sys::reboot::{RebootMode, reboot};
use nix::sys::signal::{self, Signal};
use nix::sys::wait::{WaitPidFlag, WaitStatus, waitpid};
use nix::unistd::Pid;
use sima_proto::ServiceInfo;
use spdlog::{error, info, warn};
use std::collections::HashMap;
use std::os::unix::process::CommandExt;
use std::process::Command;
use std::time::Duration;
use tokio::signal::unix::Signal as TokioSignal;
use tokio::signal::unix::{SignalKind, signal as tokio_signal};
use tokio::sync::mpsc;
use tokio::time::timeout;

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum ServiceStatus {
    Running,
    Stopped,
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
                Self::launch_service(name, config, state, &mut self.pid_map);
            }
        }
        self.event_loop().await
    }

    fn launch_service(
        name: &str,
        config: &ServiceConfig,
        state: &mut ServiceState,
        pid_map: &mut HashMap<Pid, String>,
    ) {
        if state.status == ServiceStatus::Running {
            info!("Service {} is already running", name);
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
            }
        }
    }

    fn stop_service(&mut self, name: &str) {
        let Some(state) = self.states.get(name) else {
            warn!("Service {} not found", name);
            return;
        };

        let Some(pid) = state.pid else {
            info!("Service {} is not running", name);
            return;
        };

        info!("Stopping service: {} (PID: {})", name, pid);
        let pgid = Pid::from_raw(-pid.as_raw());
        if let Err(e) = signal::kill(pgid, Signal::SIGTERM) {
            if e != nix::Error::ESRCH {
                warn!("Failed to send SIGTERM to {}: {}", name, e);
            }
        }
    }

    fn start_service(&mut self, name: &str) {
        let Some(config) = self.configs.get(name).cloned() else {
            warn!("Service {} not found in config", name);
            return;
        };

        let Some(state) = self.states.get_mut(name) else {
            return;
        };

        Self::launch_service(name, &config, state, &mut self.pid_map);
    }

    fn restart_service(&mut self, name: &str) {
        self.stop_service(name);
        // Note: actual restart happens after process exits and reap_zombies is called
        // For immediate restart, we'd need to track pending restarts
        // For now, just start it (if already stopped, it will start; if running, stop was sent)
        self.start_service(name);
    }

    fn get_status(&self) -> Vec<ServiceInfo> {
        self.configs
            .keys()
            .map(|name| {
                let state = self.states.get(name);
                ServiceInfo {
                    name: name.clone(),
                    pid: state.and_then(|s| s.pid).map(|p| p.as_raw()),
                    running: state
                        .map(|s| s.status == ServiceStatus::Running)
                        .unwrap_or(false),
                }
            })
            .collect()
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
            info!("Service {} (PID {}) exited: {:?}", name, pid, status);

            if let Some(state) = self.states.get_mut(&name) {
                state.pid = None;
                state.status = ServiceStatus::Stopped;
            }
        } else {
            info!("Reaped orphan process PID {} ({:?})", pid, status);
        }
    }

    async fn event_loop(&mut self) -> Result<()> {
        let (cmd_tx, mut cmd_rx) = mpsc::channel::<IpcCommand>(32);
        let ipc_server = IpcServer::new()?;

        let mut sigchld = tokio_signal(SignalKind::child())?;
        let mut sigterm = tokio_signal(SignalKind::terminate())?;
        let mut sigint = tokio_signal(SignalKind::interrupt())?;

        info!("Sima event loop started.");

        loop {
            tokio::select! {
                _ = sigchld.recv() => {
                    self.reap_zombies();
                }
                result = ipc_server.accept() => {
                    if let Ok(stream) = result {
                        if let Err(e) = handle_client(stream, &cmd_tx).await {
                            error!("IPC client error: {}", e);
                        }
                    }
                }
                Some(cmd) = cmd_rx.recv() => {
                    if self.handle_ipc_command(cmd, &mut sigchld).await {
                        break;
                    }
                }
                _ = sigterm.recv() => {
                    info!("Received SIGTERM, shutting down...");
                    self.perform_shutdown(&mut sigchld).await;
                    break;
                }
                _ = sigint.recv() => {
                    info!("Received SIGINT, shutting down...");
                    self.perform_shutdown(&mut sigchld).await;
                    break;
                }
            }
        }
        Ok(())
    }

    /// Returns true if event loop should exit
    async fn handle_ipc_command(&mut self, cmd: IpcCommand, sigchld: &mut TokioSignal) -> bool {
        match cmd {
            IpcCommand::Start(name) => {
                self.start_service(&name);
                false
            }
            IpcCommand::Stop(name) => {
                self.stop_service(&name);
                false
            }
            IpcCommand::Restart(name) => {
                self.restart_service(&name);
                false
            }
            IpcCommand::Status(tx) => {
                let _ = tx.send(self.get_status());
                false
            }
            IpcCommand::Poweroff => {
                info!("Poweroff requested via IPC");
                self.perform_shutdown(sigchld).await;
                let _ = reboot(RebootMode::RB_POWER_OFF);
                true
            }
            IpcCommand::Reboot => {
                info!("Reboot requested via IPC");
                self.perform_shutdown(sigchld).await;
                let _ = reboot(RebootMode::RB_AUTOBOOT);
                true
            }
            IpcCommand::SoftReboot => {
                info!("Soft-reboot requested via IPC");
                self.perform_shutdown(sigchld).await;
                self.exec_self();
            }
        }
    }

    fn exec_self(&self) -> ! {
        let exe = std::env::current_exe().unwrap_or_else(|_| "/sbin/sima-init".into());
        let err = Command::new(&exe).exec();
        panic!("soft-reboot exec failed: {}", err);
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
