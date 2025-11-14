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
    /// Write bytes to a temporary file for processing
    fn write_temp_file(data: &[u8], extension: &str) -> Result<PathBuf, AppError> {
        let temp_dir = std::env::temp_dir();
        let filename = format!("{}.{}", Uuid::new_v4(), extension);
        let temp_path = temp_dir.join(filename);

        fs::write(&temp_path, data)
            .map_err(|e| AppError::internal_error(format!("Failed to write temp file: {}", e)))?;

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
            // Word documents
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document" // .docx
                | "application/msword" // .doc
                | "application/rtf" // .rtf
                | "text/rtf" // .rtf
                | "application/vnd.oasis.opendocument.text" // .odt
            // Spreadsheets
                | "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet" // .xlsx
                | "application/vnd.ms-excel" // .xls
                | "application/vnd.oasis.opendocument.spreadsheet" // .ods
            // Presentations (text extraction from notes)
                | "application/vnd.openxmlformats-officedocument.presentationml.presentation" // .pptx
                | "application/vnd.ms-powerpoint" // .ppt
        )
    }

    async fn extract_text(&self, data: &[u8], mime_type: &str) -> Result<Option<String>, AppError> {
        let extension = Self::extension_from_mime(mime_type)
            .ok_or_else(|| AppError::internal_error("Unsupported office document type"))?;

        match mime_type {
            // Word documents - use Pandoc for markdown conversion
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
            | "application/msword"
            | "application/rtf"
            | "text/rtf"
            | "application/vnd.oasis.opendocument.text" => {
                let temp_path = Self::write_temp_file(data, extension)?;

                let result = pandoc::convert_to_markdown(&temp_path, mime_type).await;

                Self::cleanup_temp_file(&temp_path);

                match result {
                    Ok(markdown) => {
                        tracing::info!("Extracted {} characters from {} document", markdown.len(), extension);
                        Ok(Some(markdown))
                    }
                    Err(e) => {
                        tracing::warn!("Failed to extract text from {} document: {}", extension, e);
                        Ok(None)
                    }
                }
            }

            // Spreadsheets - use calamine for CSV conversion
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet" => {
                let temp_path = Self::write_temp_file(data, extension)?;

                let result = spreadsheet::convert_xlsx_to_text(&temp_path);

                Self::cleanup_temp_file(&temp_path);

                match result {
                    Ok(text) => {
                        tracing::info!("Extracted {} characters from XLSX spreadsheet", text.len());
                        Ok(Some(text))
                    }
                    Err(e) => {
                        tracing::warn!("Failed to extract text from XLSX: {}", e);
                        Ok(None)
                    }
                }
            }

            "application/vnd.ms-excel" => {
                let temp_path = Self::write_temp_file(data, extension)?;

                let result = spreadsheet::convert_xls_to_text(&temp_path);

                Self::cleanup_temp_file(&temp_path);

                match result {
                    Ok(text) => {
                        tracing::info!("Extracted {} characters from XLS spreadsheet", text.len());
                        Ok(Some(text))
                    }
                    Err(e) => {
                        tracing::warn!("Failed to extract text from XLS: {}", e);
                        Ok(None)
                    }
                }
            }

            "application/vnd.oasis.opendocument.spreadsheet" => {
                let temp_path = Self::write_temp_file(data, extension)?;

                let result = spreadsheet::convert_ods_to_text(&temp_path);

                Self::cleanup_temp_file(&temp_path);

                match result {
                    Ok(text) => {
                        tracing::info!("Extracted {} characters from ODS spreadsheet", text.len());
                        Ok(Some(text))
                    }
                    Err(e) => {
                        tracing::warn!("Failed to extract text from ODS: {}", e);
                        Ok(None)
                    }
                }
            }

            // Presentations - use Pandoc to extract speaker notes
            "application/vnd.openxmlformats-officedocument.presentationml.presentation"
            | "application/vnd.ms-powerpoint" => {
                let temp_path = Self::write_temp_file(data, extension)?;

                let result = pandoc::convert_to_markdown(&temp_path, mime_type).await;

                Self::cleanup_temp_file(&temp_path);

                match result {
                    Ok(markdown) => {
                        if markdown.trim().is_empty() {
                            tracing::info!("No speaker notes found in {} presentation", extension);
                            Ok(None)
                        } else {
                            tracing::info!("Extracted {} characters from {} presentation", markdown.len(), extension);
                            Ok(Some(markdown))
                        }
                    }
                    Err(e) => {
                        tracing::warn!("Failed to extract text from {} presentation: {}", extension, e);
                        Ok(None)
                    }
                }
            }

            _ => Ok(None),
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

            "application/vnd.openxmlformats-officedocument.presentationml.presentation"
            | "application/vnd.ms-powerpoint" => "presentation",

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
            // Word documents - can generate page images via PDF
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
                | "application/msword"
                | "application/rtf"
                | "text/rtf"
                | "application/vnd.oasis.opendocument.text"
            // Presentations - can generate slide images via PDF
                | "application/vnd.openxmlformats-officedocument.presentationml.presentation"
                | "application/vnd.ms-powerpoint"
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

        // Write temp file for Pandoc processing
        let temp_path = Self::write_temp_file(data, extension)?;

        // Create temp directory for PDF output
        let temp_dir = std::env::temp_dir().join(format!("office_pdf_{}", Uuid::new_v4()));
        fs::create_dir_all(&temp_dir)
            .map_err(|e| AppError::internal_error(format!("Failed to create temp dir: {}", e)))?;

        let temp_pdf = temp_dir.join("document.pdf");

        // Convert to PDF using Pandoc with layout options
        let result = pandoc::convert_to_pdf(&temp_path, &temp_pdf).await;

        // Clean up source file
        Self::cleanup_temp_file(&temp_path);

        match result {
            Ok(_) => {
                // Read the generated PDF
                let pdf_data = fs::read(&temp_pdf)
                    .map_err(|e| AppError::internal_error(format!("Failed to read generated PDF: {}", e)))?;

                // Use PDF processor to generate images
                let pdf_processor = PdfProcessor;
                let processing_result = pdf_processor
                    .generate_images(&pdf_data, "application/pdf", max_thumbnails)
                    .await;

                // Clean up temp directory
                let _ = fs::remove_dir_all(&temp_dir);

                processing_result
            }
            Err(e) => {
                tracing::warn!("Failed to convert {} to PDF: {}", extension, e);

                // Clean up temp directory
                let _ = fs::remove_dir_all(&temp_dir);

                Ok(ProcessingResult::default())
            }
        }
    }
}
