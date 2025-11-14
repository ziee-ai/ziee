// Text file processor

use super::traits::ContentProcessor;
use crate::common::AppError;
use async_trait::async_trait;

/// Plain text processor
pub struct TextProcessor;

#[async_trait]
impl ContentProcessor for TextProcessor {
    fn can_process(&self, mime_type: &str) -> bool {
        matches!(
            mime_type,
            "text/plain" | "text/markdown" | "text/csv" | "text/html" | "text/xml"
                | "application/json" | "application/xml"
        )
    }

    async fn extract_text(&self, data: &[u8], _mime_type: &str) -> Result<Option<String>, AppError> {
        // Try UTF-8 decoding
        match String::from_utf8(data.to_vec()) {
            Ok(text) => Ok(Some(text)),
            Err(_) => {
                // Try lossy conversion for non-UTF8 text
                Ok(Some(String::from_utf8_lossy(data).to_string()))
            }
        }
    }

    async fn extract_metadata(
        &self,
        data: &[u8],
        mime_type: &str,
    ) -> Result<serde_json::Value, AppError> {
        let text_length = data.len();

        Ok(serde_json::json!({
            "text_length": text_length,
            "format": mime_type,
            "has_text": true,
        }))
    }
}
