// PDF file processor

use super::traits::{ContentProcessor, ImageGenerator};
use super::ProcessingResult;
use crate::common::AppError;
use crate::modules::file::models::ProcessingMetadata;
use crate::modules::file::utils::pdfium::with_pdfium;
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
        

        result.replace("\n\n", "\n").trim().to_string()
    }

    /// Capture per-page citation geometry: for each page, the raw character
    /// stream + each character's bounding box, fraction-normalized to the page
    /// (origin top-left). Serialized per page as JSON `{"text": "...",
    /// "boxes": [[x,y,w,h], ...]}` (one box per char in `text`). The `text-rects`
    /// endpoint aligns a chunk's cleaned span to `text` (whitespace-insensitive)
    /// and returns the matching boxes. Best-effort: a page that errors yields an
    /// empty `{}` entry (page-level fallback).
    pub async fn extract_geometry_pages(&self, data: &[u8]) -> Result<Vec<String>, AppError> {
        let data = data.to_vec();
        tokio::task::spawn_blocking(move || {
            with_pdfium(move |pdfium| {
                let document = pdfium
                    .load_pdf_from_byte_slice(&data, None)
                    .map_err(|e| AppError::internal_with_id(e))?;

                let mut pages_json = Vec::new();
                for page_index in 0..document.pages().len() {
                    let page = match document.pages().get(page_index) {
                        Ok(p) => p,
                        Err(_) => {
                            pages_json.push("{}".to_string());
                            continue;
                        }
                    };
                    let pw = page.width().value.max(1.0);
                    let ph = page.height().value.max(1.0);
                    let text = match page.text() {
                        Ok(t) => t,
                        Err(_) => {
                            pages_json.push("{}".to_string());
                            continue;
                        }
                    };

                    let mut raw = String::new();
                    let mut boxes: Vec<[f32; 4]> = Vec::new();
                    for ch in text.chars().iter() {
                        let c = ch.unicode_char().unwrap_or('\u{FFFD}');
                        raw.push(c);
                        // tight_bounds returns a rect in PDF points (origin
                        // bottom-left). Normalize to top-left fractions.
                        let b = ch.tight_bounds().ok();
                        let rect = match b {
                            Some(r) => {
                                let x = (r.left().value / pw).clamp(0.0, 1.0);
                                let w = ((r.right().value - r.left().value) / pw).clamp(0.0, 1.0);
                                let y = ((ph - r.top().value) / ph).clamp(0.0, 1.0);
                                let h = ((r.top().value - r.bottom().value) / ph).clamp(0.0, 1.0);
                                [x, y, w, h]
                            }
                            None => [0.0, 0.0, 0.0, 0.0],
                        };
                        boxes.push(rect);
                    }

                    let val = serde_json::json!({ "text": raw, "boxes": boxes });
                    pages_json.push(val.to_string());
                }
                Ok(pages_json)
            })
        })
        .await
        .map_err(|e| AppError::internal_with_id(e))?
    }
}

#[async_trait]
impl ContentProcessor for PdfProcessor {
    fn can_process(&self, mime_type: &str) -> bool {
        mime_type == "application/pdf"
    }

    async fn extract_text(&self, data: &[u8], _mime_type: &str) -> Result<Vec<String>, AppError> {
        // PDFium FFI is synchronous and CPU-bound; run it off the async
        // runtime so it cannot block executor threads.
        let data = data.to_vec();
        tokio::task::spawn_blocking(move || {
            let processor = PdfProcessor;

            with_pdfium(move |pdfium| {
                // Load PDF document
                let document = pdfium
                    .load_pdf_from_byte_slice(&data, None)
                    .map_err(|e| AppError::internal_with_id(e))?;

                let mut text_pages = Vec::new();

                // Extract text from each page
                for page_index in 0..document.pages().len() {
                    let page = document.pages().get(page_index).map_err(AppError::internal_with_id)?;

                    let page_text = page.text().map_err(AppError::internal_with_id)?;

                    let all_text = page_text.all();
                    let cleaned_text = processor.clean_extracted_text(&all_text);

                    text_pages.push(cleaned_text);
                }

                Ok(text_pages)
            })
        })
        .await
        .map_err(|e| AppError::internal_with_id(e))?
    }

    async fn extract_metadata(
        &self,
        data: &[u8],
        mime_type: &str,
    ) -> Result<serde_json::Value, AppError> {
        // Try to get page count via PDFium. The FFI is synchronous and
        // CPU-bound, so run it off the async runtime.
        let data_owned = data.to_vec();
        let page_count = tokio::task::spawn_blocking(move || {
            with_pdfium(move |pdfium| {
                Ok(pdfium
                    .load_pdf_from_byte_slice(&data_owned, None)
                    .ok()
                    .map(|document| document.pages().len() as u32))
            })
            .unwrap_or(None)
        })
        .await
        .map_err(|e| AppError::internal_with_id(e))?;

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
        // PDFium rendering is synchronous and CPU-bound; run it off the
        // async runtime so it cannot block executor threads.
        let data = data.to_vec();
        tokio::task::spawn_blocking(move || {
        with_pdfium(move |pdfium| {
        // Load the PDF document from bytes
        let document = pdfium
            .load_pdf_from_byte_slice(&data, None)
            .map_err(|e| AppError::internal_with_id(e))?;

        let page_count = document.pages().len() as u32;
        let max_pages = page_count.min(max_thumbnails);

        // Generate all preview images at full size
        let mut images = Vec::new();
        for page_index in 0..max_pages {
            let page = document
                .pages()
                .get(page_index as u16)
                .map_err(AppError::internal_with_id)?;

            // Generate high-quality image (2000px)
            let image_bytes = render_page_to_jpeg(&page, MAX_IMAGE_DIM)?;
            images.push(image_bytes);
        }

        // Generate single 300px thumbnail from first page only
        let thumbnails = if let Ok(first_page) = document.pages().get(0) {
            vec![render_page_to_jpeg(&first_page, 300)?]
        } else {
            vec![]
        };

        let metadata = ProcessingMetadata {
            has_text: Some(true),
            // The doc's actual page count, NOT the rendered image
            // count. The frontend reads this alongside the file's
            // `preview_page_count` to detect truncation (we cap
            // rendered pages at `PREVIEW_PAGE_CAP` in
            // processing/mod.rs) and surface a "showing first N of
            // M" banner. Without this, the frontend has no way to
            // know how many pages the doc actually has.
            page_count: Some(page_count),
            ..Default::default()
        };

        Ok(ProcessingResult {
            text_pages: vec![], // Text is extracted separately via ContentProcessor
            geometry_pages: vec![],
            metadata,
            thumbnails, // Single element array
            images,     // Multiple elements (one per page)
        })
        })
        })
        .await
        .map_err(|e| AppError::internal_with_id(e))?
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
        .map_err(|e| AppError::internal_with_id(e))?;

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
        .map_err(|e| AppError::internal_with_id(e))?;

    Ok(buffer)
}
