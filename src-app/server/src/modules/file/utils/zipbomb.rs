//! Decompression-bomb pre-validation for ZIP-family uploads.
//!
//! OOXML containers (DOCX/XLSX/PPTX) and ODT/ODS are ZIP archives.
//! Without a pre-check, a small 16 MB upload can claim to expand to
//! tens of GB and exhaust memory when the office processor opens it.
//! Closes 05-file F-05 (High).
//!
//! Caller passes the raw upload bytes; we walk the central directory
//! and:
//!   - sum the declared uncompressed sizes
//!   - check the worst per-entry compression ratio
//! Returns Err if either limit is exceeded. We do NOT decompress
//! anything — the central-directory metadata is enough.

use std::io::Cursor;

/// Max total uncompressed size across all entries (256 MiB).
pub const MAX_UNCOMPRESSED_BYTES: u64 = 256 * 1024 * 1024;

/// Max ratio uncompressed/compressed per entry. Real DOCX/XLSX
/// typically compress at < 10:1; 200:1 leaves headroom for embedded
/// XML but flags the classic zip bomb.
pub const MAX_COMPRESSION_RATIO: u64 = 200;

#[derive(Debug)]
pub enum ZipBombError {
    OpenFailed(String),
    TotalSizeExceeded { declared: u64, cap: u64 },
    RatioExceeded { ratio: u64, cap: u64 },
}

impl std::fmt::Display for ZipBombError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::OpenFailed(e) => write!(f, "Cannot open as ZIP archive: {}", e),
            Self::TotalSizeExceeded { declared, cap } => write!(
                f,
                "ZIP declares {} bytes uncompressed (cap is {})",
                declared, cap
            ),
            Self::RatioExceeded { ratio, cap } => write!(
                f,
                "ZIP entry compression ratio {}:1 exceeds cap of {}:1 \
                 (likely zip bomb)",
                ratio, cap
            ),
        }
    }
}

impl std::error::Error for ZipBombError {}

/// Validate a ZIP-family upload against the configured caps. Returns
/// Ok(()) if safe to hand to the office processor.
pub fn validate(bytes: &[u8]) -> Result<(), ZipBombError> {
    let cursor = Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(cursor)
        .map_err(|e| ZipBombError::OpenFailed(e.to_string()))?;

    let mut total: u64 = 0;
    for i in 0..archive.len() {
        let file = archive
            .by_index_raw(i)
            .map_err(|e| ZipBombError::OpenFailed(format!("entry {}: {}", i, e)))?;
        let uncompressed = file.size();
        let compressed = file.compressed_size().max(1); // /0 guard
        let ratio = uncompressed / compressed;
        if ratio > MAX_COMPRESSION_RATIO {
            return Err(ZipBombError::RatioExceeded {
                ratio,
                cap: MAX_COMPRESSION_RATIO,
            });
        }
        total = total.saturating_add(uncompressed);
        if total > MAX_UNCOMPRESSED_BYTES {
            return Err(ZipBombError::TotalSizeExceeded {
                declared: total,
                cap: MAX_UNCOMPRESSED_BYTES,
            });
        }
    }

    Ok(())
}

/// MIME prefixes that are ZIP-family containers. The processor pipeline
/// calls `validate` before extraction for any matching mime_type.
pub fn is_ooxml_or_odf(mime_type: &str) -> bool {
    matches!(
        mime_type,
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
            | "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
            | "application/vnd.openxmlformats-officedocument.presentationml.presentation"
            | "application/vnd.oasis.opendocument.text"
            | "application/vnd.oasis.opendocument.spreadsheet"
            | "application/zip"
    )
}
