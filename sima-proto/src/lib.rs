use serde::{Deserialize, Serialize};
use std::io;

pub const PRIMARY_SOCKET_PATH: &str = "/run/sima.sock";
pub const FALLBACK_SOCKET_PATH: &str = "/tmp/sima.sock";

pub fn socket_paths() -> [&'static str; 2] {
    [PRIMARY_SOCKET_PATH, FALLBACK_SOCKET_PATH]
}

pub fn should_fallback_from_socket_error(err: &io::Error) -> bool {
    matches!(
        err.kind(),
        io::ErrorKind::PermissionDenied
            | io::ErrorKind::NotFound
            | io::ErrorKind::ReadOnlyFilesystem
    )
}

pub fn should_fallback_from_connect_error(err: &io::Error) -> bool {
    matches!(
        err.kind(),
        io::ErrorKind::NotFound | io::ErrorKind::ConnectionRefused
    )
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Request {
    Start(String),
    Stop(String),
    Restart(String),
    Status,
    Poweroff,
    Reboot,
    SoftReboot,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Response {
    Ok,
    Error(String),
    StatusReport(Vec<ServiceInfo>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceInfo {
    pub name: String,
    pub pid: Option<i32>,
    pub running: bool,
}

pub fn encode<T: Serialize>(msg: &T) -> Result<Vec<u8>, postcard::Error> {
    postcard::to_stdvec(msg)
}

pub fn decode<'a, T: Deserialize<'a>>(bytes: &'a [u8]) -> Result<T, postcard::Error> {
    postcard::from_bytes(bytes)
}
