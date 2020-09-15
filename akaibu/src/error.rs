use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AkaibuError {
    #[error("Unrecognized format: {0} {1:X?}")]
    UnrecognizedFormat(PathBuf, Vec<u8>),
    #[error("Unimplemented: {0}")]
    Unimplemented(String),
    #[error("{0}")]
    Custom(String),
    #[error("Unknown error")]
    Unknown,
}
