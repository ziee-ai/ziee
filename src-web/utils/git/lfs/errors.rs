use thiserror::Error;

#[derive(Debug, Error)]
pub enum LfsError {
    #[error("Remote server responded with 401 or 403")]
    AccessDenied,

    #[error("Remote server responded with not-okay code: {0}")]
    ResponseNotOkay(String),

    #[error("File IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Could not parse file: {0}")]
    InvalidFormat(&'static str),

    #[error("Request error: {0}")]
    RequestError(#[from] reqwest::Error),

    #[error("Remote file not found: {0}")]
    RemoteFileNotFound(&'static str),

    #[error("Checksum incorrect")]
    ChecksumMismatch,

    #[error("Could not decode oid-string to bytes: {0}")]
    OidNotValidHex(#[from] hex::FromHexError),

    #[error("Problem traversing directory structure: {0}")]
    DirectoryTraversalError(String),

    #[error("Could not parse remote URL: {0}")]
    UrlParsingError(#[from] url::ParseError),

    #[error("Invalid header value: {0}")]
    InvalidHeaderValue(#[from] http::header::InvalidHeaderValue),

    #[error("HTTP error: {0}")]
    HTTP(#[from] http::Error),

    #[error("Invalid HTTP response: {0}")]
    InvalidResponse(String),

    #[error("TempFile error: {0}")]
    TempFile(String),

    #[error("Operation was cancelled")]
    Cancelled,
}

impl From<&'static str> for LfsError {
    fn from(message: &'static str) -> Self {
        LfsError::InvalidFormat(message)
    }
}
