use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("No nearby points found for snapping")]
    NoPointsFound,
    #[error("Invalid node index")]
    InvalidNodeIndex,
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Invalid data: {0}")]
    InvalidData(String),
    #[error("Isochrone error: {0}")]
    IsochroneError(String),
    #[error("H3 error: {0}")]
    H3Error(#[from] h3o::error::InvalidGeometry),
    #[error("Unknown error: {0}")]
    UnrecoverableError(&'static str),
    #[error("Invalid time format: {0}")]
    InvalidTimeFormat(String),
}
