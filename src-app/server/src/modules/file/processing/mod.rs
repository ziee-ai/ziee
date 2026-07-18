// File processing system

pub mod traits;
pub mod text;
pub mod text_image;
pub mod spreadsheet_image;
pub mod image;
pub mod pdf;
pub mod office;

use crate::common::AppError;
use traits::{ContentProcessor, ImageGenerator};

// The pure-data `ProcessingResult` moved to the `ziee-file` SDK crate (chunk
// `ziee-file`); re-exported here so every `super::ProcessingResult` /
// `processing::ProcessingResult` path (this module's submodules + upload/
// versioning) resolves unchanged. The PRODUCERS below (ProcessingManager +
// the per-format processors) stay app-side.
pub use crate::modules::file::models::ProcessingResult;

/// Maximum number of pages we rasterize into preview-page images at
/// upload time. A 200-page PDF at the 2000px-per-page render size
/// would consume ~50-100 MB of disk for preview alone; cap keeps the
/// blast radius bounded. When the doc has more pages than the cap,
/// `ProcessingMetadata::page_count` retains the true total and the
/// frontend surfaces a "showing first N of M pages" banner.
pub const PREVIEW_PAGE_CAP: u32 = 50;

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
            // OfficeProcessor renders DOCX / DOC / RTF / ODT by piping
            // them through Pandoc → typst → PDF, then handing the PDF
            // back to PdfProcessor::generate_images for per-page raster.
            // PPTX / PPT are intentionally not supported (pandoc 3.x
            // doesn't read PowerPoint as input). Without this line the
            // office-doc preview-page count stays at zero at upload
            // time and the PDF viewer shows the "preview not
            // available" empty state.
            Box::new(office::OfficeProcessor),
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
                // Citation geometry: per-char boxes for the exact-passage
                // highlight. PDFs capture directly; Office docs via their PDF
                // render; everything else returns none (page-level fallback).
                // Best-effort — a failure just opens the page without a box.
                result.geometry_pages =
                    processor.extract_geometry(data, mime_type).await.unwrap_or_else(|e| {
                        tracing::warn!("geometry: extract_geometry failed: {e}");
                        Vec::new()
                    });
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
                let image_result = generator.generate_images(data, mime_type, PREVIEW_PAGE_CAP).await?;
                result.thumbnails = image_result.thumbnails;
                result.images = image_result.images;

                // Merge metadata
                if result.metadata.width.is_none() {
                    result.metadata.width = image_result.metadata.width;
                    result.metadata.height = image_result.metadata.height;
                    result.metadata.format = image_result.metadata.format;
                }
                // `page_count` lives on the image_result for paged
                // formats (PDF / DOCX-via-PDF) because the page count
                // is only known after rendering. Office docs don't
                // populate it in `extract_metadata` (which sees only
                // the source bytes), so we merge it in here even when
                // the content-processor side already set the metadata.
                if result.metadata.page_count.is_none() {
                    result.metadata.page_count = image_result.metadata.page_count;
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

    /// Geometry-only extraction for the backfill pass — find the processor for
    /// `mime_type` and capture per-page citation geometry. Empty on any failure
    /// or when no processor handles the type (page-level fallback).
    pub async fn geometry_pages(&self, data: &[u8], mime_type: &str) -> Vec<String> {
        for processor in &self.content_processors {
            if processor.can_process(mime_type) {
                return processor
                    .extract_geometry(data, mime_type)
                    .await
                    .unwrap_or_default();
            }
        }
        Vec::new()
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


    // ----- Graceful degradation for failed processing (audit all-f2c43a4939b6) -----
    //
    // The upload handler (file/handlers/upload.rs:165-168) maps a `process_file`
    // Err to `ProcessingResult::default()` so an unprocessable file still uploads
    // (with an empty/degraded result) instead of failing the whole request. Prior
    // processing tests only covered happy paths; these cover the error path the
    // handler relies on and the safe-empty value it degrades to.

    #[tokio::test]
    async fn corrupt_spreadsheet_surfaces_processing_error() {
        // Bytes that claim to be an .xlsx but are not a valid OOXML/zip
        // workbook: the spreadsheet image generator's Calamine
        // `open_workbook_from_rs` fails and `process_file` propagates the Err.
        // Calamine is pure-Rust (no external binary / runtime dylib), so the
        // failure is deterministic — this is exactly the Err the upload handler
        // catches and degrades to `ProcessingResult::default()`.
        const XLSX: &str =
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet";
        // Leading NUL keeps the text-fallback heuristic from claiming it as text;
        // the bytes are not a zip, so the workbook open fails.
        let not_a_workbook = b"\x00\x01 corrupted, not a real xlsx workbook \x00";
        let result = ProcessingManager::new()
            .process_file(not_a_workbook, XLSX)
            .await;
        assert!(
            result.is_err(),
            "a corrupt spreadsheet must surface a processing error for the upload \
             handler to map to a degraded ProcessingResult::default()"
        );
    }


    #[test]
    fn processing_result_default_is_safe_empty_degradation() {
        // The value the upload handler falls back to on a processing error:
        // graceful degradation == a valid-but-empty result (file still stored,
        // just nothing extracted), never partial/garbage content.
        let degraded = ProcessingResult::default();
        assert!(degraded.text_pages.is_empty(), "no text pages");
        assert!(degraded.thumbnails.is_empty(), "no thumbnails");
        assert!(degraded.images.is_empty(), "no preview images");
        assert_eq!(degraded.metadata.has_text, None, "has_text unset");
        assert_eq!(degraded.metadata.text_length, None, "text_length unset");
    }


    /// Graceful degradation: the upload path falls back to
    /// `ProcessingResult::default()` when processing fails, so the default MUST
    /// be a safe no-artifacts result (empty text pages / thumbnails / images) —
    /// the file is still stored, just with nothing derived.
    #[test]
    fn processing_result_default_is_empty_no_artifacts() {
        let r = ProcessingResult::default();
        assert!(r.text_pages.is_empty());
        assert!(r.thumbnails.is_empty());
        assert!(r.images.is_empty());
    }


    /// A corrupt file that CLAIMS a rich mime (PDF) but is garbage bytes must
    /// degrade gracefully — no panic, and a safe result (no text pages / no
    /// thumbnails) rather than crashing the upload. Exercises the failure path
    /// that feeds the `ProcessingResult::default()` fallback.
    #[tokio::test]
    async fn corrupt_pdf_degrades_without_panic() {
        let garbage = b"%PDF-1.7\nthis is not a real pdf body \x00\x01\x02 garbage";
        let outcome = ProcessingManager::new()
            .process_file(garbage, "application/pdf")
            .await;
        // Either an Err (caller falls back to default) OR an Ok degraded result
        // with no extracted artifacts — both are graceful (no panic).
        match outcome {
            Ok(r) => {
                assert!(r.thumbnails.is_empty(), "corrupt pdf yields no thumbnail");
                // text_pages may be empty or a best-effort scrap; must not panic.
                let _ = r.text_pages;
            }
            Err(_) => {}
        }
    }
}
