// Office document processor

use super::pdf::PdfProcessor;
use super::traits::{ContentProcessor, ImageGenerator};
use super::ProcessingResult;
use crate::common::AppError;
use crate::modules::file::utils::{pandoc, spreadsheet};
use async_trait::async_trait;
use std::fs;
use std::path::PathBuf;
use uuid::Uuid;

/// Office document processor
pub struct OfficeProcessor;

impl OfficeProcessor {
    /// Write bytes to a temporary file for processing.
    ///
    /// SECURITY: writes the file with owner-only permissions (mode 0600)
    /// so other local users / processes on the host can't read the
    /// in-flight upload while we're processing it. The previous
    /// `fs::write` used the umask-default mode (typically 0644 → world-
    /// readable on Linux). Closes 05-file F-10 + F-11 (Medium).
    fn write_temp_file(data: &[u8], extension: &str) -> Result<PathBuf, AppError> {
        let temp_dir = std::env::temp_dir();
        let filename = format!("{}.{}", Uuid::new_v4(), extension);
        let temp_path = temp_dir.join(filename);

        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&temp_path)
                .map_err(|e| {
                    AppError::internal_with_id(e)
                })?;
            use std::io::Write;
            file.write_all(data).map_err(|e| {
                AppError::internal_with_id(e)
            })?;
        }
        #[cfg(not(unix))]
        {
            fs::write(&temp_path, data).map_err(|e| {
                AppError::internal_with_id(e)
            })?;
        }

        Ok(temp_path)
    }

    /// Clean up temporary file
    fn cleanup_temp_file(path: &PathBuf) {
        if let Err(e) = fs::remove_file(path) {
            tracing::warn!("Failed to clean up temp file {:?}: {}", path, e);
        }
    }

    /// Detect file extension from MIME type
    fn extension_from_mime(mime_type: &str) -> Option<&str> {
        match mime_type {
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document" => Some("docx"),
            "application/msword" => Some("doc"),
            "application/rtf" | "text/rtf" => Some("rtf"),
            "application/vnd.oasis.opendocument.text" => Some("odt"),
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet" => Some("xlsx"),
            "application/vnd.ms-excel" => Some("xls"),
            "application/vnd.oasis.opendocument.spreadsheet" => Some("ods"),
            "application/vnd.openxmlformats-officedocument.presentationml.presentation" => Some("pptx"),
            "application/vnd.ms-powerpoint" => Some("ppt"),
            _ => None,
        }
    }
}

#[async_trait]
impl ContentProcessor for OfficeProcessor {
    fn can_process(&self, mime_type: &str) -> bool {
        matches!(
            mime_type,
            // Word documents (pandoc → typst path)
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document" // .docx
                | "application/msword" // .doc
                | "application/rtf" // .rtf
                | "text/rtf" // .rtf
                | "application/vnd.oasis.opendocument.text" // .odt
            // Spreadsheets (per-sheet text via Calamine)
                | "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet" // .xlsx
                | "application/vnd.ms-excel" // .xls
                | "application/vnd.oasis.opendocument.spreadsheet" // .ods
            // NOTE: PPTX / PPT are NOT supported. Pandoc 3.x cannot
            // read PowerPoint formats as INPUT; office2pdf 0.6.0
            // (the only pure-Rust PPTX renderer found) is published
            // broken against quick-xml 0.38.4. Reach back here when
            // a viable converter exists.
        )
    }

    async fn extract_text(&self, data: &[u8], mime_type: &str) -> Result<Vec<String>, AppError> {
        let extension = Self::extension_from_mime(mime_type)
            .ok_or_else(|| AppError::internal_error("Unsupported office document type"))?;

        match mime_type {
            // Word documents - convert to PDF then extract text per-page
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
            | "application/msword"
            | "application/rtf"
            | "text/rtf"
            | "application/vnd.oasis.opendocument.text" => {
                let temp_path = Self::write_temp_file(data, extension)?;

                // Create temp directory for PDF output
                let temp_dir = std::env::temp_dir().join(format!("office_text_pdf_{}", Uuid::new_v4()));
                fs::create_dir_all(&temp_dir)
                    .map_err(|e| AppError::internal_with_id(e))?;

                let temp_pdf = temp_dir.join("document.pdf");

                // Convert to PDF using Pandoc
                let result = pandoc::convert_to_pdf(&temp_path, &temp_pdf).await;

                // Clean up source file
                Self::cleanup_temp_file(&temp_path);

                match result {
                    Ok(_) => {
                        // Read the generated PDF
                        let pdf_data = fs::read(&temp_pdf)
                            .map_err(|e| {
                                let _ = fs::remove_dir_all(&temp_dir);
                                AppError::internal_with_id(e)
                            })?;

                        // Use PDF processor to extract text per-page
                        let pdf_processor = PdfProcessor;
                        let text_pages = pdf_processor.extract_text(&pdf_data, "application/pdf").await;

                        // Clean up temp directory
                        let _ = fs::remove_dir_all(&temp_dir);

                        match text_pages {
                            Ok(pages) => {
                                let total_chars: usize = pages.iter().map(|p| p.len()).sum();
                                tracing::info!("Extracted {} pages ({} total chars) from {} document via PDF conversion", pages.len(), total_chars, extension);
                                Ok(pages)
                            }
                            Err(e) => {
                                tracing::warn!("Failed to extract text from {} PDF: {}", extension, e);
                                Ok(vec![])
                            }
                        }
                    }
                    Err(e) => {
                        tracing::warn!("Failed to convert {} to PDF: {}", extension, e);
                        let _ = fs::remove_dir_all(&temp_dir);
                        Ok(vec![])
                    }
                }
            }

            // Spreadsheets - extract per-sheet text
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet" => {
                let temp_path = Self::write_temp_file(data, extension)?;

                let result = spreadsheet::convert_xlsx_to_pages(&temp_path);

                Self::cleanup_temp_file(&temp_path);

                match result {
                    Ok(pages) => {
                        let total_chars: usize = pages.iter().map(|p| p.len()).sum();
                        tracing::info!("Extracted {} pages ({} total chars) from XLSX spreadsheet", pages.len(), total_chars);
                        Ok(pages)
                    }
                    Err(e) => {
                        tracing::warn!("Failed to extract text from XLSX: {}", e);
                        Ok(vec![])
                    }
                }
            }

            "application/vnd.ms-excel" => {
                let temp_path = Self::write_temp_file(data, extension)?;

                let result = spreadsheet::convert_xls_to_pages(&temp_path);

                Self::cleanup_temp_file(&temp_path);

                match result {
                    Ok(pages) => {
                        let total_chars: usize = pages.iter().map(|p| p.len()).sum();
                        tracing::info!("Extracted {} pages ({} total chars) from XLS spreadsheet", pages.len(), total_chars);
                        Ok(pages)
                    }
                    Err(e) => {
                        tracing::warn!("Failed to extract text from XLS: {}", e);
                        Ok(vec![])
                    }
                }
            }

            "application/vnd.oasis.opendocument.spreadsheet" => {
                let temp_path = Self::write_temp_file(data, extension)?;

                let result = spreadsheet::convert_ods_to_pages(&temp_path);

                Self::cleanup_temp_file(&temp_path);

                match result {
                    Ok(pages) => {
                        let total_chars: usize = pages.iter().map(|p| p.len()).sum();
                        tracing::info!("Extracted {} pages ({} total chars) from ODS spreadsheet", pages.len(), total_chars);
                        Ok(pages)
                    }
                    Err(e) => {
                        tracing::warn!("Failed to extract text from ODS: {}", e);
                        Ok(vec![])
                    }
                }
            }

            _ => Ok(vec![]),
        }
    }

    /// Per-page citation geometry for Word-style docs — captured from the same
    /// PDF render used for text (page-aligned), so office citations get an
    /// exact-passage highlight too. Spreadsheets/other → none (page-level).
    async fn extract_geometry(
        &self,
        data: &[u8],
        mime_type: &str,
    ) -> Result<Vec<String>, AppError> {
        let extension = match Self::extension_from_mime(mime_type) {
            Some(e) => e,
            None => return Ok(vec![]),
        };
        match mime_type {
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
            | "application/msword"
            | "application/rtf"
            | "text/rtf"
            | "application/vnd.oasis.opendocument.text" => {
                let temp_path = Self::write_temp_file(data, extension)?;
                let temp_dir =
                    std::env::temp_dir().join(format!("office_geom_pdf_{}", Uuid::new_v4()));
                if let Err(e) = fs::create_dir_all(&temp_dir) {
                    // Don't leak the temp input file on the mkdir error path.
                    Self::cleanup_temp_file(&temp_path);
                    return Err(AppError::internal_with_id(e));
                }
                let temp_pdf = temp_dir.join("document.pdf");
                let converted = pandoc::convert_to_pdf(&temp_path, &temp_pdf).await;
                Self::cleanup_temp_file(&temp_path);
                let geometry = match converted {
                    Ok(_) => match fs::read(&temp_pdf) {
                        Ok(pdf_data) => PdfProcessor
                            .extract_geometry_pages(&pdf_data)
                            .await
                            .unwrap_or_default(),
                        Err(_) => vec![],
                    },
                    Err(e) => {
                        tracing::warn!("geometry: {} PDF conversion failed: {e}", extension);
                        vec![]
                    }
                };
                let _ = fs::remove_dir_all(&temp_dir);
                Ok(geometry)
            }
            _ => Ok(vec![]),
        }
    }

    async fn extract_metadata(
        &self,
        data: &[u8],
        mime_type: &str,
    ) -> Result<serde_json::Value, AppError> {
        let extension = Self::extension_from_mime(mime_type).unwrap_or("unknown");

        let doc_type = match mime_type {
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
            | "application/msword"
            | "application/rtf"
            | "text/rtf"
            | "application/vnd.oasis.opendocument.text" => "word_document",

            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
            | "application/vnd.ms-excel"
            | "application/vnd.oasis.opendocument.spreadsheet" => "spreadsheet",

            _ => "office_document",
        };

        Ok(serde_json::json!({
            "type": doc_type,
            "file_size": data.len(),
            "format": extension
        }))
    }
}

#[async_trait]
impl ImageGenerator for OfficeProcessor {
    fn can_generate(&self, mime_type: &str) -> bool {
        matches!(
            mime_type,
            // Word documents (pandoc → typst → PDF)
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
                | "application/msword"
                | "application/rtf"
                | "text/rtf"
                | "application/vnd.oasis.opendocument.text"
            // PPTX / PPT not listed — see the note in `can_process()`.
        )
    }

    async fn generate_images(
        &self,
        data: &[u8],
        mime_type: &str,
        max_thumbnails: u32,
    ) -> Result<ProcessingResult, AppError> {
        let extension = Self::extension_from_mime(mime_type)
            .ok_or_else(|| AppError::internal_error("Unsupported office document type"))?;

        let temp_path = Self::write_temp_file(data, extension)?;
        let temp_dir = std::env::temp_dir().join(format!("office_pdf_{}", Uuid::new_v4()));
        fs::create_dir_all(&temp_dir).map_err(|e| {
            AppError::internal_with_id(e)
        })?;
        let temp_pdf = temp_dir.join("document.pdf");

        let result = pandoc::convert_to_pdf(&temp_path, &temp_pdf).await;

        Self::cleanup_temp_file(&temp_path);

        match result {
            Ok(_) => {
                let pdf_data = fs::read(&temp_pdf).map_err(|e| {
                    let _ = fs::remove_dir_all(&temp_dir);
                    AppError::internal_with_id(e)
                })?;

                let processing_result = PdfProcessor
                    .generate_images(&pdf_data, "application/pdf", max_thumbnails)
                    .await;

                let _ = fs::remove_dir_all(&temp_dir);
                processing_result
            }
            Err(e) => {
                tracing::warn!("Failed to convert {} to PDF: {}", extension, e);
                let _ = fs::remove_dir_all(&temp_dir);
                Ok(ProcessingResult::default())
            }
        }
    }
}

#[cfg(test)]
mod geometry_tests {
    use super::*;
    use crate::modules::file::processing::traits::ContentProcessor;

    // TEST-57 (FB-9 / ITEM-22): OfficeProcessor::extract_geometry returns EMPTY
    // for a spreadsheet mime (no Word→PDF render → page-level fallback), and the
    // trait DEFAULT is empty (TextProcessor path). Deterministic, no external deps.
    #[tokio::test]
    async fn office_geometry_empty_for_spreadsheet() {
        let g = OfficeProcessor
            .extract_geometry(
                b"x",
                "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
            )
            .await
            .unwrap();
        assert!(g.is_empty(), "spreadsheet geometry must be empty (page-level fallback)");
    }

    #[tokio::test]
    async fn text_processor_geometry_default_empty() {
        let g = super::super::text::TextProcessor
            .extract_geometry(b"hello", "text/plain")
            .await
            .unwrap();
        assert!(g.is_empty(), "the trait default returns no geometry");
    }
}
