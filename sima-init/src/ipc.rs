use anyhow::Result;
use sima_proto::{
    Request, Response, ServiceInfo, decode, encode, should_fallback_from_socket_error, socket_paths,
};
use spdlog::{error, info, warn};
use std::fs;
use std::io;
use std::os::unix::fs::FileTypeExt;
use std::os::unix::net::UnixStream as StdUnixStream;
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
    socket_path: String,
}

impl IpcServer {
    pub fn new() -> Result<Self> {
        let (listener, socket_path) = bind_listener(&socket_paths())?;
        info!("IPC server listening on {}", socket_path);
        Ok(Self {
            listener,
            socket_path,
        })
    }

    pub async fn accept(&self) -> Result<UnixStream> {
        let (stream, _) = self.listener.accept().await?;
        Ok(stream)
    }
}

impl Drop for IpcServer {
    fn drop(&mut self) {
        if let Err(err) = fs::remove_file(&self.socket_path)
            && err.kind() != io::ErrorKind::NotFound
        {
            warn!("Failed to remove IPC socket {}: {}", self.socket_path, err);
        }
    }
}

fn bind_listener(paths: &[&str]) -> Result<(UnixListener, String)> {
    bind_listener_with(paths, bind_socket).map_err(Into::into)
}

fn bind_listener_with<T>(
    paths: &[&str],
    mut bind: impl FnMut(&str) -> io::Result<T>,
) -> io::Result<(T, String)> {
    let mut last_error = None;

    for (index, socket_path) in paths.iter().copied().enumerate() {
        match bind(socket_path) {
            Ok(listener) => return Ok((listener, socket_path.to_string())),
            Err(err) => {
                let has_fallback = index + 1 < paths.len();
                if has_fallback && should_fallback_from_socket_error(&err) {
                    warn!(
                        "Failed to bind IPC socket at {}: {}. Trying fallback path.",
                        socket_path, err
                    );
                    last_error = Some(err);
                    continue;
                }
                return Err(err.into());
            }
        }
    }

    match last_error {
        Some(err) => Err(err),
        None => Err(io::Error::other("no IPC socket paths configured")),
    }
}

fn bind_socket(socket_path: &str) -> io::Result<UnixListener> {
    cleanup_stale_socket(socket_path)?;
    UnixListener::bind(socket_path)
}

fn cleanup_stale_socket(socket_path: &str) -> io::Result<()> {
    let metadata = match fs::symlink_metadata(socket_path) {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(err) => return Err(err),
    };

    if !metadata.file_type().is_socket() {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!("{socket_path} exists and is not a socket"),
        ));
    }

    match StdUnixStream::connect(socket_path) {
        Ok(_) => {
            return Err(io::Error::new(
                io::ErrorKind::AddrInUse,
                format!("{socket_path} is already in use"),
            ));
        }
        Err(err)
            if matches!(
                err.kind(),
                io::ErrorKind::ConnectionRefused | io::ErrorKind::NotFound
            ) => {}
        Err(err) => return Err(err),
    }

    fs::remove_file(socket_path)
}

pub async fn handle_client(
    mut stream: UnixStream,
    cmd_tx: &mpsc::Sender<IpcCommand>,
) -> Result<()> {
    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).await?;
    if buf.is_empty() {
        return Ok(());
    }

    let req: Request = match decode(&buf) {
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

#[cfg(test)]
mod tests {
    use super::bind_listener_with;
    use std::io;

    #[test]
    fn bind_listener_falls_back_on_permission_denied() {
        let (_listener, socket_path) =
            bind_listener_with(&["/run/sima.sock", "/tmp/sima.sock"], |socket_path| {
                match socket_path {
                    "/run/sima.sock" => Err::<(), io::Error>(io::Error::new(
                        io::ErrorKind::PermissionDenied,
                        "read-only filesystem",
                    )),
                    "/tmp/sima.sock" => Ok::<(), io::Error>(()),
                    _ => unreachable!("unexpected socket path"),
                }
            })
            .expect("bind should succeed");

        assert_eq!(socket_path, "/tmp/sima.sock");
    }

    #[test]
    fn bind_listener_does_not_fall_back_on_addr_in_use() {
        let err = bind_listener_with(&["/run/sima.sock", "/tmp/sima.sock"], |_socket_path| {
            Err::<(), io::Error>(io::Error::new(
                io::ErrorKind::AddrInUse,
                "socket already in use",
            ))
        })
        .expect_err("bind should fail");

        assert_eq!(err.kind(), io::ErrorKind::AddrInUse);
    }
}
