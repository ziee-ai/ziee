// File processing system

pub mod traits;
pub mod text;
pub mod text_image;
pub mod spreadsheet_image;
pub mod image;
pub mod pdf;
pub mod office;

use crate::common::AppError;
use crate::modules::file::models::ProcessingMetadata;
use traits::{ContentProcessor, ImageGenerator};

/// Processing result
#[derive(Debug, Clone, Default)]
pub struct ProcessingResult {
    pub text_pages: Vec<String>,
    pub metadata: ProcessingMetadata,
    pub thumbnails: Vec<Vec<u8>>,
    pub images: Vec<Vec<u8>>,
}

/// File processing manager
pub struct ProcessingManager {
    content_processors: Vec<Box<dyn ContentProcessor>>,
    image_generators: Vec<Box<dyn ImageGenerator>>,
}

impl ProcessingManager {
    /// Create new processing manager with default processors
    pub fn new() -> Self {
        let content_processors: Vec<Box<dyn ContentProcessor>> = vec![
            Box::new(text::TextProcessor),
            Box::new(pdf::PdfProcessor),
            Box::new(office::OfficeProcessor),
        ];

        let image_generators: Vec<Box<dyn ImageGenerator>> = vec![
            Box::new(image::ImageProcessor),
            Box::new(pdf::PdfProcessor),
            Box::new(spreadsheet_image::SpreadsheetImageGenerator),
            Box::new(text_image::TextImageGenerator),
        ];

        Self {
            content_processors,
            image_generators,
        }
    }

    /// Process file and extract content
    pub async fn process_file(
        &self,
        data: &[u8],
        mime_type: &str,
    ) -> Result<ProcessingResult, AppError> {
        let mut result = ProcessingResult::default();

        // Extract text content
        for processor in &self.content_processors {
            if processor.can_process(mime_type) {
                result.text_pages = processor.extract_text(data, mime_type).await?;
                let metadata_json = processor.extract_metadata(data, mime_type).await?;
                result.metadata = serde_json::from_value(metadata_json)
                    .unwrap_or_default();
                break;
            }
        }

        // Fallback: no content processor claimed this MIME, but the bytes look
        // like plain text. This covers source-code / config files whose
        // `mime_guess` MIME isn't in any processor's allow-list (e.g. `.R` →
        // application/octet-stream, `.rs` → text/x-rust, `.sql`, `.sh`, `.yaml`,
        // …). Without this they'd persist with `text_page_count = 0` and the
        // `/files/{id}/text` endpoint would return an empty body, so the text
        // viewer renders nothing. We reuse TextProcessor (a plain UTF-8 decode
        // into one page); the guard keeps true binaries at zero text pages.
        if result.text_pages.is_empty() && looks_like_text(data) {
            result.text_pages = text::TextProcessor.extract_text(data, mime_type).await?;
        }

        // Generate images
        for generator in &self.image_generators {
            if generator.can_generate(mime_type) {
                let image_result = generator.generate_images(data, mime_type, 5).await?;
                result.thumbnails = image_result.thumbnails;
                result.images = image_result.images;

                // Merge metadata
                if result.metadata.width.is_none() {
                    result.metadata.width = image_result.metadata.width;
                    result.metadata.height = image_result.metadata.height;
                    result.metadata.format = image_result.metadata.format;
                }
                break;
            }
        }

        // Update metadata
        if !result.text_pages.is_empty() {
            let total_text_length: usize = result.text_pages.iter().map(|s| s.len()).sum();
            result.metadata.text_length = Some(total_text_length);
            result.metadata.has_text = Some(true);
        }

        Ok(result)
    }
}

impl Default for ProcessingManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Heuristic: does this byte slice look like decodable plain text (vs binary)?
///
/// Used by the text-extraction fallback in `process_file`. Conservative on
/// purpose — a false negative just means a text file isn't previewable (still
/// downloadable); a false positive would render binary garbage in the viewer.
///
/// Rules: non-empty, under a generous size cap, no NUL bytes (the single
/// strongest binary signal), and either valid UTF-8 or lossy-decodes with very
/// few replacement characters (tolerates the odd latin-1 byte).
fn looks_like_text(data: &[u8]) -> bool {
    // 10 MiB — code/config artifacts are far smaller; avoids decoding a huge
    // blob just to throw most of it away (the viewer truncates anyway).
    const MAX_SNIFF_BYTES: usize = 10 * 1024 * 1024;

    if data.is_empty() || data.len() > MAX_SNIFF_BYTES {
        return false;
    }
    if data.contains(&0) {
        return false;
    }
    match std::str::from_utf8(data) {
        Ok(_) => true,
        Err(_) => {
            // Allow a small fraction of undecodable bytes (e.g. latin-1 text).
            let lossy = String::from_utf8_lossy(data);
            let total = lossy.chars().count().max(1);
            let replacements = lossy.matches('\u{FFFD}').count();
            replacements * 20 < total // < 5% replacement chars
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn r_script_octet_stream_extracts_text() {
        // `.R` resolves to application/octet-stream (no mime_guess mapping),
        // so no content processor claims it — the fallback must kick in.
        let src = "x <- c(1, 2, 3)\nprint(mean(x))\n";
        let result = ProcessingManager::new()
            .process_file(src.as_bytes(), "application/octet-stream")
            .await
            .unwrap();
        assert_eq!(result.text_pages.len(), 1, "R script should yield one text page");
        assert_eq!(result.text_pages[0], src);
    }

    #[tokio::test]
    async fn code_mimes_outside_allowlist_extract_text() {
        // text/x-rust, application/x-sql, text/javascript etc. are NOT in any
        // ContentProcessor allow-list but are plain text.
        for (body, mime) in [
            ("fn main() {}\n", "text/x-rust"),
            ("SELECT 1;\n", "application/x-sql"),
            ("console.log(1)\n", "text/javascript"),
            ("key: value\n", "text/x-yaml"),
        ] {
            let result = ProcessingManager::new()
                .process_file(body.as_bytes(), mime)
                .await
                .unwrap();
            assert_eq!(result.text_pages, vec![body.to_string()], "mime {mime} should extract text");
        }
    }

    #[tokio::test]
    async fn binary_data_yields_no_text_pages() {
        // PNG magic + a NUL byte → must stay binary (no garbage text page).
        let png = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x01, 0x02];
        let result = ProcessingManager::new()
            .process_file(&png, "application/octet-stream")
            .await
            .unwrap();
        assert!(result.text_pages.is_empty(), "binary data must not be extracted as text");
    }

    #[tokio::test]
    async fn csv_still_extracted_by_text_processor() {
        // Regression: the explicit text/csv path is unchanged.
        let csv = "a,b\n1,2\n";
        let result = ProcessingManager::new()
            .process_file(csv.as_bytes(), "text/csv")
            .await
            .unwrap();
        assert_eq!(result.text_pages, vec![csv.to_string()]);
    }

    #[test]
    fn looks_like_text_guards() {
        assert!(looks_like_text(b"plain text"));
        assert!(!looks_like_text(b""));
        assert!(!looks_like_text(b"has\0nul"));
        assert!(!looks_like_text(&[0xff, 0xfe, 0xfd, 0x00]));
    }
}
