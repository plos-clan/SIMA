use std::io;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SimaError {
    #[error("sima I/O error")]
    IoError(#[from] io::Error),

    #[error("config format error")]
    FormatError(#[from] serde_yaml::Error),
}

pub type SimaResult<T> = Result<T, SimaError>;
