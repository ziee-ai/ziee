// PDF file processor

use super::traits::{ContentProcessor, ImageGenerator};
use super::ProcessingResult;
use crate::common::AppError;
use crate::modules::file::models::ProcessingMetadata;
use crate::modules::file::utils::pdfium::init_pdfium;
use async_trait::async_trait;
use image::{imageops, ImageBuffer, RgbImage};
use pdfium_render::prelude::*;
use std::collections::HashSet;

const MAX_IMAGE_DIM: u32 = 2000;

/// PDF processor
pub struct PdfProcessor;

impl PdfProcessor {
    /// Clean up extracted text by removing excessive whitespace and normalizing line breaks
    fn clean_extracted_text(&self, text: &str) -> String {
        let lines: Vec<&str> = text.lines().collect();
        let mut cleaned_lines = Vec::new();
        let mut seen_lines = HashSet::new();

        for line in lines {
            let trimmed = line.trim();

            // Skip empty lines and very short lines that are likely artifacts
            if trimmed.is_empty() || trimmed.len() < 2 {
                continue;
            }

            // Skip duplicate lines (common in PDFs with headers/footers)
            if seen_lines.contains(trimmed) {
                continue;
            }

            seen_lines.insert(trimmed.to_string());
            cleaned_lines.push(trimmed);
        }

        // Join lines with proper spacing
        let result = cleaned_lines.join("\n");

        // Remove excessive whitespace
        let result = result.split_whitespace().collect::<Vec<&str>>().join(" ");

        // Restore paragraph breaks by looking for sentence endings
        let result = result
            .replace(". ", ".\n")
            .replace("! ", "!\n")
            .replace("? ", "?\n");

        // Clean up any double newlines
        let result = result.replace("\n\n", "\n").trim().to_string();

        result
    }
}

#[async_trait]
impl ContentProcessor for PdfProcessor {
    fn can_process(&self, mime_type: &str) -> bool {
        mime_type == "application/pdf"
    }

    async fn extract_text(&self, data: &[u8], _mime_type: &str) -> Result<Option<String>, AppError> {
        // Extract text from PDF bytes using pdf-extract
        let data_owned = data.to_vec();
        let extracted_text = tokio::task::spawn_blocking(move || {
            pdf_extract::extract_text_from_mem(&data_owned)
        })
        .await
        .map_err(|e| AppError::internal_error(format!("Task join error: {}", e)))?
        .map_err(|e| AppError::internal_error(format!("PDF text extraction failed: {}", e)))?;

        // Clean up the extracted text
        let cleaned_text = self.clean_extracted_text(&extracted_text);

        if cleaned_text.trim().is_empty() {
            Ok(None)
        } else {
            Ok(Some(cleaned_text))
        }
    }

    async fn extract_metadata(
        &self,
        data: &[u8],
        mime_type: &str,
    ) -> Result<serde_json::Value, AppError> {
        // Try to get page count via PDFium
        let page_count = match init_pdfium() {
            Ok(pdfium) => {
                match pdfium.load_pdf_from_byte_slice(data, None) {
                    Ok(document) => Some(document.pages().len() as u32),
                    Err(_) => None,
                }
            }
            Err(_) => None,
        };

        Ok(serde_json::json!({
            "format": mime_type,
            "has_text": true,
            "page_count": page_count,
        }))
    }
}

#[async_trait]
impl ImageGenerator for PdfProcessor {
    fn can_generate(&self, mime_type: &str) -> bool {
        mime_type == "application/pdf"
    }

    async fn generate_images(
        &self,
        data: &[u8],
        _mime_type: &str,
        max_thumbnails: u32,
    ) -> Result<ProcessingResult, AppError> {
        // Initialize PDFium
        let pdfium = init_pdfium()
            .map_err(|e| AppError::internal_error(format!("PDFium initialization failed: {}", e)))?;

        // Load the PDF document from bytes
        let document = pdfium
            .load_pdf_from_byte_slice(data, None)
            .map_err(|e| AppError::internal_error(format!("Failed to load PDF: {}", e)))?;

        let page_count = document.pages().len() as u32;
        let max_pages = page_count.min(max_thumbnails);

        let mut thumbnails = Vec::new();
        let mut images = Vec::new();

        // Generate images for each page
        for page_index in 0..max_pages {
            let page = document
                .pages()
                .get(page_index as u16)
                .map_err(|e| AppError::internal_error(format!("Failed to get page {}: {}", page_index + 1, e)))?;

            // Generate thumbnail (300px)
            let thumbnail_bytes = render_page_to_jpeg(&page, 300)?;
            thumbnails.push(thumbnail_bytes);

            // Generate high-quality image (2000px)
            let image_bytes = render_page_to_jpeg(&page, MAX_IMAGE_DIM)?;
            images.push(image_bytes);
        }

        let metadata = ProcessingMetadata {
            has_text: Some(true),
            ..Default::default()
        };

        Ok(ProcessingResult {
            text_content: None, // Text is extracted separately via ContentProcessor
            metadata,
            thumbnails,
            images,
        })
    }
}

fn render_page_to_jpeg(page: &PdfPage, max_dim: u32) -> Result<Vec<u8>, AppError> {
    let effective_max_dim = max_dim.min(MAX_IMAGE_DIM);
    let render_config = PdfRenderConfig::new()
        .set_target_width(effective_max_dim as i32)
        .set_maximum_height(effective_max_dim as i32)
        .rotate_if_landscape(PdfPageRenderRotation::Degrees90, true);

    let bitmap = page
        .render_with_config(&render_config)
        .map_err(|e| AppError::internal_error(format!("Failed to render page: {}", e)))?;

    // Convert bitmap to RGB image
    let width = bitmap.width() as u32;
    let height = bitmap.height() as u32;
    let pixel_data = bitmap.as_raw_bytes();

    // Convert BGRA to RGB
    let mut rgb_data = Vec::with_capacity((width * height * 3) as usize);
    for pixel in pixel_data.chunks_exact(4) {
        rgb_data.push(pixel[2]); // R (from B in BGRA)
        rgb_data.push(pixel[1]); // G
        rgb_data.push(pixel[0]); // B (from R in BGRA)
                                 // Skip alpha channel
    }

    // Create RGB image
    let mut rgb_image: RgbImage = ImageBuffer::from_raw(width, height, rgb_data)
        .ok_or_else(|| AppError::internal_error("Failed to create RGB image from raw data"))?;

    // Handle landscape page rotation
    if page.is_landscape() {
        rgb_image = imageops::rotate270(&rgb_image);
    }

    // Encode as JPEG
    let mut buffer = Vec::new();
    let mut cursor = std::io::Cursor::new(&mut buffer);
    rgb_image
        .write_to(&mut cursor, image::ImageFormat::Jpeg)
        .map_err(|e| AppError::internal_error(format!("Failed to encode JPEG: {}", e)))?;

    Ok(buffer)
}
