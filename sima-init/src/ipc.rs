use anyhow::Result;
use sima_proto::{Request, Response, ServiceInfo, SOCKET_PATH, decode, encode};
use spdlog::{error, info};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::{mpsc, oneshot};

pub enum IpcCommand {
    Start(String),
    Stop(String),
    Restart(String),
    Status(oneshot::Sender<Vec<ServiceInfo>>),
    Poweroff,
    Reboot,
    SoftReboot,
}

pub struct IpcServer {
    listener: UnixListener,
}

impl IpcServer {
    pub fn new() -> Result<Self> {
        let _ = std::fs::remove_file(SOCKET_PATH);
        let listener = UnixListener::bind(SOCKET_PATH)?;
        info!("IPC server listening on {}", SOCKET_PATH);
        Ok(Self { listener })
    }

    pub async fn accept(&self) -> Result<UnixStream> {
        let (stream, _) = self.listener.accept().await?;
        Ok(stream)
    }
}

pub async fn handle_client(
    mut stream: UnixStream,
    cmd_tx: &mpsc::Sender<IpcCommand>,
) -> Result<()> {
    let mut buf = vec![0u8; 4096];
    let n = stream.read(&mut buf).await?;
    if n == 0 {
        return Ok(());
    }

    let req: Request = match decode(&buf[..n]) {
        Ok(r) => r,
        Err(e) => {
            error!("Failed to decode IPC request: {}", e);
            let resp = Response::Error(format!("Invalid request: {}", e));
            let data = encode(&resp)?;
            stream.write_all(&data).await?;
            return Ok(());
        }
    };

    info!("IPC request: {:?}", req);
    let resp = process_request(req, cmd_tx).await;

    let data = encode(&resp)?;
    stream.write_all(&data).await?;
    Ok(())
}

async fn process_request(req: Request, cmd_tx: &mpsc::Sender<IpcCommand>) -> Response {
    match req {
        Request::Start(name) => {
            if cmd_tx.send(IpcCommand::Start(name)).await.is_err() {
                return Response::Error("Internal error".into());
            }
            Response::Ok
        }
        Request::Stop(name) => {
            if cmd_tx.send(IpcCommand::Stop(name)).await.is_err() {
                return Response::Error("Internal error".into());
            }
            Response::Ok
        }
        Request::Restart(name) => {
            if cmd_tx.send(IpcCommand::Restart(name)).await.is_err() {
                return Response::Error("Internal error".into());
            }
            Response::Ok
        }
        Request::Status => {
            let (tx, rx) = oneshot::channel();
            if cmd_tx.send(IpcCommand::Status(tx)).await.is_err() {
                return Response::Error("Internal error".into());
            }
            match rx.await {
                Ok(statuses) => Response::StatusReport(statuses),
                Err(_) => Response::Error("Failed to get status".into()),
            }
        }
        Request::Poweroff => {
            if cmd_tx.send(IpcCommand::Poweroff).await.is_err() {
                return Response::Error("Internal error".into());
            }
            Response::Ok
        }
        Request::Reboot => {
            if cmd_tx.send(IpcCommand::Reboot).await.is_err() {
                return Response::Error("Internal error".into());
            }
            Response::Ok
        }
        Request::SoftReboot => {
            if cmd_tx.send(IpcCommand::SoftReboot).await.is_err() {
                return Response::Error("Internal error".into());
            }
            Response::Ok
        }
    }
}
