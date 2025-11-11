use serde::Serialize;
use std::fmt::{Display, Formatter};

/// This enum specifies the source of the file that has been placed inside the repository.
#[derive(Debug, PartialEq, Eq, Copy, Clone, Serialize)]
pub enum FilePullMode {
    /// Remote was used
    DownloadedFromRemote,
    /// Local git-lfs cache was used
    UsedLocalCache,
    /// File was already pulled
    WasAlreadyPresent,
}

impl Display for FilePullMode {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            FilePullMode::DownloadedFromRemote => write!(f, "Downloaded from lfs server"),
            FilePullMode::UsedLocalCache => write!(f, "Taken from local cache"),
            FilePullMode::WasAlreadyPresent => write!(f, "File already pulled"),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct LfsProgress {
    pub phase: LfsPhase,
    pub current: u64,
    pub total: u64,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub enum LfsPhase {
    Scanning,
    Downloading,
    Complete,
    Error,
}
