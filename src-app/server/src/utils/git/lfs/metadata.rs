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

        // Validate oid against the LFS spec: SHA-256 over hex is exactly
        // 64 lowercase hex characters. Without this, an attacker-controlled
        // LFS pointer file with oid="../../../etc/passwd" lets the cache
        // path / temp file path escape the repo cache directory
        // (07-llm-model F-02 High) — and oid[0..2]/oid[2..4] also panics
        // on OIDs shorter than 4 chars (07-llm-model F-02 + F-06).
        if !is_valid_oid(oid) {
            return Err(LfsError::InvalidFormat(
                "LFS pointer oid must be exactly 64 lowercase hex characters",
            ));
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

/// Validate that an LFS oid matches the SHA-256-hex format that the
/// LFS spec mandates (64 lowercase hex chars). Closes 07-llm-model F-02.
fn is_valid_oid(oid: &str) -> bool {
    oid.len() == 64 && oid.chars().all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c))
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
            // Same validation as parse_from_string — reject anything that
            // isn't 64 lowercase hex chars before letting the caller use
            // the value as a path component. Closes 07-llm-model F-02.
            if is_valid_oid(oid_value) {
                oid = Some(oid_value.to_string());
            } else {
                return None;
            }
        } else if let Some(size_str) = line.strip_prefix("size ")
            && let Ok(size_value) = size_str.parse::<u64>() {
                size = Some(size_value);
            }
    }

    match (oid, size) {
        (Some(o), Some(s)) => Some((o, s)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pointer(oid: &str) -> String {
        format!(
            "version https://git-lfs.github.com/spec/v1\noid sha256:{}\nsize 12345\n",
            oid
        )
    }

    #[test]
    fn rejects_oid_with_path_traversal() {
        let malicious = "../../../etc/passwd";
        let err = LfsMetadata::parse_from_string(&pointer(malicious))
            .expect_err("path-traversal oid must be rejected");
        match err {
            LfsError::InvalidFormat(_) => {}
            other => panic!("expected InvalidFormat, got {other:?}"),
        }
    }

    #[test]
    fn rejects_short_oid() {
        // Audit also flagged that oid[0..2]/oid[2..4] panics for OIDs
        // shorter than 4 chars; the length check at parse time prevents
        // that downstream panic entirely.
        let err = LfsMetadata::parse_from_string(&pointer("abc"))
            .expect_err("short oid must be rejected");
        assert!(matches!(err, LfsError::InvalidFormat(_)));
    }

    #[test]
    fn rejects_oid_with_uppercase() {
        // The LFS spec mandates lowercase hex.
        let oid_upper = "ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789";
        let err = LfsMetadata::parse_from_string(&pointer(oid_upper))
            .expect_err("uppercase hex must be rejected");
        assert!(matches!(err, LfsError::InvalidFormat(_)));
    }

    #[test]
    fn rejects_oid_with_non_hex_chars() {
        let oid_bad = "g123456789012345678901234567890123456789012345678901234567890123";
        let err = LfsMetadata::parse_from_string(&pointer(oid_bad))
            .expect_err("non-hex char must be rejected");
        assert!(matches!(err, LfsError::InvalidFormat(_)));
    }

    #[test]
    fn accepts_valid_oid() {
        let valid = "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789";
        let m = LfsMetadata::parse_from_string(&pointer(valid))
            .expect("valid SHA-256 hex must be accepted");
        assert_eq!(m.oid, valid);
    }

    #[test]
    fn parse_lfs_pointer_content_rejects_path_traversal() {
        assert!(parse_lfs_pointer_content(&pointer("../../etc/passwd")).is_none());
    }

    #[test]
    fn parse_lfs_pointer_content_accepts_valid() {
        let valid = "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789";
        let r = parse_lfs_pointer_content(&pointer(valid));
        assert_eq!(r, Some((valid.to_string(), 12345)));
    }
}
