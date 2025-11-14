// Spreadsheet image generation - creates preview images from spreadsheet files
// Each sheet is rendered as a separate page image

use async_trait::async_trait;
use calamine::{open_workbook_from_rs, Ods, Reader, Xls, Xlsx};
use image::{ImageBuffer, Rgb, RgbImage};
use imageproc::drawing::{draw_line_segment_mut, draw_text_mut};
use ab_glyph::{FontRef, PxScale};
use std::io::Cursor;

use crate::common::AppError;
use super::{ProcessingResult, traits::ImageGenerator};

const IMAGE_WIDTH: u32 = 1200;
const IMAGE_HEIGHT: u32 = 900;
const MARGIN: u32 = 20;
const CELL_HEIGHT: u32 = 25;
const CELL_WIDTH: u32 = 120;
const FONT_SIZE: f32 = 12.0;
const MAX_ROWS_PER_PAGE: usize = 30;
const MAX_COLS_PER_PAGE: usize = 9;

pub struct SpreadsheetImageGenerator;

impl SpreadsheetImageGenerator {
    /// Resize image to thumbnail size (300px max dimension)
    fn resize_to_thumbnail(image_data: &[u8]) -> Result<Vec<u8>, AppError> {
        const THUMBNAIL_SIZE: u32 = 300;

        // Decode JPEG
        let img = image::load_from_memory(image_data)
            .map_err(|e| AppError::internal_error(format!("Failed to decode image: {}", e)))?;

        // Calculate dimensions maintaining aspect ratio
        let (width, height) = img.dimensions();
        let (new_width, new_height) = if width > height {
            (THUMBNAIL_SIZE, (height * THUMBNAIL_SIZE) / width)
        } else {
            ((width * THUMBNAIL_SIZE) / height, THUMBNAIL_SIZE)
        };

        // Resize with Lanczos3 filter (high quality) - resize returns an ImageBuffer
        let resized = image::imageops::resize(
            &img,
            new_width,
            new_height,
            image::imageops::FilterType::Lanczos3
        );

        // Convert to DynamicImage for encoding
        let thumbnail = image::DynamicImage::ImageRgb8(resized);

        // Encode to JPEG
        let mut jpeg_data = Vec::new();
        thumbnail.write_to(&mut std::io::Cursor::new(&mut jpeg_data), image::ImageFormat::Jpeg)
            .map_err(|e| AppError::internal_error(format!("Failed to encode thumbnail: {}", e)))?;

        Ok(jpeg_data)
    }

    /// Render a sheet as a table image
    fn render_sheet_image(
        sheet_name: &str,
        rows: Vec<Vec<String>>,
        page_number: u32,
    ) -> Result<Vec<u8>, AppError> {
        // Create white background
        let mut img: RgbImage = ImageBuffer::new(IMAGE_WIDTH, IMAGE_HEIGHT);
        for pixel in img.pixels_mut() {
            *pixel = Rgb([255, 255, 255]);
        }

        // Load font
        let font_data = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/resources/fonts/DejaVuSansMono.ttf"));
        let font = FontRef::try_from_slice(font_data)
            .map_err(|e| AppError::internal_error(format!("Failed to load font: {}", e)))?;

        let scale = PxScale::from(FONT_SIZE);
        let text_color = Rgb([40, 40, 40]);
        let grid_color = Rgb([200, 200, 200]);
        let header_bg = Rgb([240, 240, 240]);

        // Draw sheet title
        let title = format!("Sheet: {} (Page {})", sheet_name, page_number);
        draw_text_mut(
            &mut img,
            Rgb([60, 60, 60]),
            MARGIN as i32,
            MARGIN as i32 / 2,
            PxScale::from(FONT_SIZE + 2.0),
            &font,
            &title,
        );

        let table_start_y = MARGIN + 30;
        let num_rows = rows.len().min(MAX_ROWS_PER_PAGE);
        let num_cols = rows.get(0).map(|r| r.len()).unwrap_or(0).min(MAX_COLS_PER_PAGE);

        // Draw grid
        let table_width = (num_cols as u32) * CELL_WIDTH;
        let table_height = (num_rows as u32) * CELL_HEIGHT;

        // Horizontal lines
        for i in 0..=num_rows {
            let y = table_start_y + (i as u32 * CELL_HEIGHT);
            draw_line_segment_mut(
                &mut img,
                (MARGIN as f32, y as f32),
                ((MARGIN + table_width) as f32, y as f32),
                grid_color,
            );
        }

        // Vertical lines
        for i in 0..=num_cols {
            let x = MARGIN + (i as u32 * CELL_WIDTH);
            draw_line_segment_mut(
                &mut img,
                (x as f32, table_start_y as f32),
                (x as f32, (table_start_y + table_height) as f32),
                grid_color,
            );
        }

        // Fill header row background (first row)
        if num_rows > 0 {
            for x in MARGIN..(MARGIN + table_width) {
                for y in table_start_y..(table_start_y + CELL_HEIGHT) {
                    if x < IMAGE_WIDTH && y < IMAGE_HEIGHT {
                        img.put_pixel(x, y, header_bg);
                    }
                }
            }
        }

        // Draw cell content
        for (row_idx, row) in rows.iter().take(MAX_ROWS_PER_PAGE).enumerate() {
            for (col_idx, cell) in row.iter().take(MAX_COLS_PER_PAGE).enumerate() {
                let x = MARGIN + (col_idx as u32 * CELL_WIDTH) + 5; // 5px padding
                let y = table_start_y + (row_idx as u32 * CELL_HEIGHT) + 5;

                // Truncate cell content to fit
                let max_chars = 15;
                let cell_text = if cell.len() > max_chars {
                    format!("{}...", &cell.chars().take(max_chars - 3).collect::<String>())
                } else {
                    cell.clone()
                };

                draw_text_mut(
                    &mut img,
                    text_color,
                    x as i32,
                    y as i32,
                    scale,
                    &font,
                    &cell_text,
                );
            }
        }

        // Add row/column indicators if truncated
        if rows.len() > MAX_ROWS_PER_PAGE || num_cols > MAX_COLS_PER_PAGE {
            let truncate_msg = format!(
                "Showing {} of {} rows, {} of {} columns",
                num_rows,
                rows.len(),
                num_cols,
                rows.get(0).map(|r| r.len()).unwrap_or(0)
            );
            draw_text_mut(
                &mut img,
                Rgb([150, 150, 150]),
                MARGIN as i32,
                (IMAGE_HEIGHT - 25) as i32,
                PxScale::from(10.0),
                &font,
                &truncate_msg,
            );
        }

        // Encode to JPEG
        let mut jpeg_data = Vec::new();
        let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg_data, 85);
        encoder
            .encode(
                &img,
                IMAGE_WIDTH,
                IMAGE_HEIGHT,
                image::ExtendedColorType::Rgb8,
            )
            .map_err(|e| AppError::internal_error(format!("Failed to encode JPEG: {}", e)))?;

        Ok(jpeg_data)
    }

    /// Extract all sheets from XLSX workbook
    fn extract_xlsx_sheets(data: &[u8]) -> Result<Vec<(String, Vec<Vec<String>>)>, AppError> {
        let cursor = Cursor::new(data);
        let mut workbook: Xlsx<_> = open_workbook_from_rs(cursor)
            .map_err(|e| AppError::internal_error(format!("Failed to open XLSX: {}", e)))?;

        let mut sheets = Vec::new();

        for sheet_name in workbook.sheet_names().to_vec() {
            if let Ok(range) = workbook.worksheet_range(&sheet_name) {
                let mut rows = Vec::new();

                for row in range.rows() {
                    let row_data: Vec<String> = row
                        .iter()
                        .map(|cell| format!("{}", cell))
                        .collect();
                    rows.push(row_data);
                }

                sheets.push((sheet_name, rows));
            }
        }

        Ok(sheets)
    }

    /// Extract all sheets from XLS workbook
    fn extract_xls_sheets(data: &[u8]) -> Result<Vec<(String, Vec<Vec<String>>)>, AppError> {
        let cursor = Cursor::new(data);
        let mut workbook: Xls<_> = open_workbook_from_rs(cursor)
            .map_err(|e| AppError::internal_error(format!("Failed to open XLS: {}", e)))?;

        let mut sheets = Vec::new();

        for sheet_name in workbook.sheet_names().to_vec() {
            if let Ok(range) = workbook.worksheet_range(&sheet_name) {
                let mut rows = Vec::new();

                for row in range.rows() {
                    let row_data: Vec<String> = row
                        .iter()
                        .map(|cell| format!("{}", cell))
                        .collect();
                    rows.push(row_data);
                }

                sheets.push((sheet_name, rows));
            }
        }

        Ok(sheets)
    }

    /// Extract all sheets from ODS workbook
    fn extract_ods_sheets(data: &[u8]) -> Result<Vec<(String, Vec<Vec<String>>)>, AppError> {
        let cursor = Cursor::new(data);
        let mut workbook: Ods<_> = open_workbook_from_rs(cursor)
            .map_err(|e| AppError::internal_error(format!("Failed to open ODS: {}", e)))?;

        let mut sheets = Vec::new();

        for sheet_name in workbook.sheet_names().to_vec() {
            if let Ok(range) = workbook.worksheet_range(&sheet_name) {
                let mut rows = Vec::new();

                for row in range.rows() {
                    let row_data: Vec<String> = row
                        .iter()
                        .map(|cell| format!("{}", cell))
                        .collect();
                    rows.push(row_data);
                }

                sheets.push((sheet_name, rows));
            }
        }

        Ok(sheets)
    }
}

#[async_trait]
impl ImageGenerator for SpreadsheetImageGenerator {
    fn can_generate(&self, mime_type: &str) -> bool {
        matches!(
            mime_type,
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet" // XLSX
                | "application/vnd.ms-excel" // XLS
                | "application/vnd.oasis.opendocument.spreadsheet" // ODS
        )
    }

    async fn generate_images(
        &self,
        data: &[u8],
        mime_type: &str,
        max_thumbnails: u32,
    ) -> Result<ProcessingResult, AppError> {
        // Extract all sheets based on MIME type
        let sheets = match mime_type {
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet" => {
                Self::extract_xlsx_sheets(data)?
            }
            "application/vnd.ms-excel" => Self::extract_xls_sheets(data)?,
            "application/vnd.oasis.opendocument.spreadsheet" => Self::extract_ods_sheets(data)?,
            _ => return Err(AppError::internal_error("Unsupported spreadsheet format")),
        };

        // Generate all sheet preview images at full size
        let mut images = Vec::new();
        for (page_num, (sheet_name, rows)) in sheets.iter().take(max_thumbnails as usize).enumerate() {
            if rows.is_empty() {
                continue; // Skip empty sheets
            }

            let page_number = (page_num + 1) as u32;
            let image_data = Self::render_sheet_image(sheet_name, rows.clone(), page_number)?;
            images.push(image_data);
        }

        // Create single 300px thumbnail from first sheet preview
        let thumbnails = if let Some(first_image) = images.first() {
            vec![Self::resize_to_thumbnail(first_image)?]
        } else {
            vec![]
        };

        Ok(ProcessingResult {
            text_content: None, // Text extraction handled by other processors
            metadata: Default::default(),
            thumbnails, // Single element array
            images,     // Multiple elements (one per sheet)
        })
    }
}
