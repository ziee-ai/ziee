use super::errors::LfsError;
use std::path::{Path, PathBuf};
use tokio::fs;

const SIZE_PREFIX: &str = "size";
const VERSION_PREFIX: &str = "version";
const OID_PREFIX: &str = "oid";
const FILE_HEADER: &str = "version https://git-lfs.github.com/spec/v1";

#[derive(PartialEq, Eq, Debug, Clone)]
pub enum Hash {
    SHA256,
    Other,
}

#[derive(Debug, Clone)]
pub struct LfsMetadata {
    pub oid: String,
    pub size: u64,
    pub hash: Option<Hash>,
}

#[derive(Debug, Clone)]
pub struct LfsPointer {
    pub size: u64,
    pub path: PathBuf,
}

impl LfsMetadata {
    /// Parse LFS metadata from file contents
    pub fn parse_from_string(input: &str) -> Result<Self, LfsError> {
        let lines: std::collections::HashMap<_, _> = input
            .lines()
            .map(|line| line.split(' ').collect::<Vec<_>>())
            .filter_map(|split_line| Some((*split_line.first()?, *split_line.last()?)))
            .collect();

        let size = lines
            .get(SIZE_PREFIX)
            .ok_or("Could not find size entry")?
            .parse::<u64>()
            .map_err(|_| "Could not convert file size to u64")?;

        let _version = *lines
            .get(VERSION_PREFIX)
            .ok_or("Could not find version-entry")?;

        let mut oid = *lines.get(OID_PREFIX).ok_or("Could not find oid-entry")?;

        let mut hash = None;
        if oid.contains(':') {
            let lines: Vec<_> = oid.split(':').collect();
            if lines.first().ok_or("Problem parsing oid entry for hash")? == &"sha256" {
                hash = Some(Hash::SHA256);
            } else {
                hash = Some(Hash::Other);
            }
            oid = *lines.last().ok_or("Problem parsing oid entry for oid")?;
        }

        Ok(LfsMetadata {
            size,
            oid: oid.to_string(),
            hash,
        })
    }

    /// Parse LFS metadata from a file
    pub async fn parse_from_file<P: AsRef<Path>>(path: P) -> Result<Self, LfsError> {
        let contents = fs::read_to_string(path).await?;
        Self::parse_from_string(&contents)
    }
}

/// Check if a file is an LFS pointer file
pub async fn is_lfs_pointer_file<P: AsRef<Path>>(path: P) -> Result<bool, LfsError> {
    if path.as_ref().is_dir() {
        return Ok(false);
    }

    let mut reader = fs::File::open(&path).await?;
    let mut buf: Vec<u8> = vec![0; FILE_HEADER.len()];

    use tokio::io::AsyncReadExt;
    let read_result = reader.read_exact(buf.as_mut_slice()).await;

    if let Err(e) = read_result {
        match e.kind() {
            std::io::ErrorKind::UnexpectedEof => Ok(false),
            _ => Err(LfsError::Io(e)),
        }
    } else {
        Ok(buf == FILE_HEADER.as_bytes())
    }
}

/// Parse LFS pointer content from string (for compatibility with existing code)
pub fn parse_lfs_pointer_content(content: &str) -> Option<(String, u64)> {
    let mut oid = None;
    let mut size = None;

    for line in content.lines() {
        if let Some(oid_value) = line.strip_prefix("oid sha256:") {
            oid = Some(oid_value.to_string());
        } else if let Some(size_str) = line.strip_prefix("size ") {
            if let Ok(size_value) = size_str.parse::<u64>() {
                size = Some(size_value);
            }
        }
    }

    match (oid, size) {
        (Some(o), Some(s)) => Some((o, s)),
        _ => None,
    }
}
