use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AkaibuError {
    #[error("Unrecognized format: {0} {1:X?}")]
    UnrecognizedFormat(PathBuf, Vec<u8>),
    #[error("Unknown error")]
    Unknown,
}
