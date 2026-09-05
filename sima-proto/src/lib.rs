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

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum Request {
    Start(String),
    Stop(String),
    Restart(String),
    Status,
    Poweroff,
    Reboot,
    SoftReboot,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum Response {
    Ok,
    Error(String),
    StatusReport(Vec<ServiceInfo>),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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

pub fn encode_request(request: &Request) -> Result<Vec<u8>, postcard::Error> {
    encode(request)
}

pub fn decode_request(bytes: &[u8]) -> Result<Request, postcard::Error> {
    decode(bytes)
}

pub fn encode_response(response: &Response) -> Result<Vec<u8>, postcard::Error> {
    encode(response)
}

pub fn decode_response(bytes: &[u8]) -> Result<Response, postcard::Error> {
    decode(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_round_trip_through_shared_codecs() {
        for request in [
            Request::Start("shell".into()),
            Request::Stop("shell".into()),
            Request::Restart("shell".into()),
            Request::Status,
            Request::Poweroff,
            Request::Reboot,
            Request::SoftReboot,
        ] {
            let bytes = encode_request(&request).expect("request should encode");
            let decoded = decode_request(&bytes).expect("request should decode");
            assert_eq!(decoded, request);
        }
    }

    #[test]
    fn responses_round_trip_through_shared_codecs() {
        for response in [
            Response::Ok,
            Response::Error("service unavailable".into()),
            Response::StatusReport(vec![]),
            Response::StatusReport(vec![
                ServiceInfo {
                    name: "shell".into(),
                    pid: Some(42),
                    running: true,
                },
                ServiceInfo {
                    name: "terminal".into(),
                    pid: None,
                    running: false,
                },
            ]),
        ] {
            let bytes = encode_response(&response).expect("response should encode");
            let decoded = decode_response(&bytes).expect("response should decode");
            assert_eq!(decoded, response);
        }
    }

    #[test]
    fn shared_codecs_preserve_the_wire_format() {
        assert_eq!(
            encode_request(&Request::Status).expect("status should encode"),
            [3]
        );
        assert_eq!(
            encode_response(&Response::Ok).expect("OK should encode"),
            [0]
        );
    }

    #[test]
    fn shared_decoders_reject_invalid_messages() {
        assert!(decode_request(&[]).is_err());
        assert!(decode_response(&[]).is_err());
        assert!(decode_request(&[127]).is_err());
        assert!(decode_response(&[127]).is_err());
    }
}
