// File processing traits

use crate::common::AppError;
use async_trait::async_trait;
use super::ProcessingResult;

/// Content processor trait for text extraction
#[async_trait]
pub trait ContentProcessor: Send + Sync {
    /// Check if this processor can handle the given MIME type
    fn can_process(&self, mime_type: &str) -> bool;

    /// Extract text content from file (per-page)
    async fn extract_text(&self, data: &[u8], mime_type: &str) -> Result<Vec<String>, AppError>;

    /// Extract metadata
    async fn extract_metadata(&self, data: &[u8], mime_type: &str) -> Result<serde_json::Value, AppError>;

    /// Per-page citation geometry (JSON strings, aligned 1:1 with `extract_text`
    /// pages) for the exact-passage highlight. Default: none (page-level
    /// fallback). PDFs implement it directly; Office docs via their PDF render.
    async fn extract_geometry(
        &self,
        _data: &[u8],
        _mime_type: &str,
    ) -> Result<Vec<String>, AppError> {
        Ok(Vec::new())
    }
}

/// Image generator trait for thumbnails and previews
#[async_trait]
pub trait ImageGenerator: Send + Sync {
    /// Check if this generator can handle the given MIME type
    fn can_generate(&self, mime_type: &str) -> bool;

    /// Generate images (both thumbnails and full-quality)
    async fn generate_images(
        &self,
        data: &[u8],
        mime_type: &str,
        max_thumbnails: u32,
    ) -> Result<ProcessingResult, AppError>;
}
