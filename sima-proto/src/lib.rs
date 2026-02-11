use serde::{Deserialize, Serialize};

pub const SOCKET_PATH: &str = "/run/sima.sock";

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
