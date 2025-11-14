// Text image generation - creates preview images from text files
// Uses advanced rendering with rusttype for better text display

use async_trait::async_trait;
use encoding_rs::*;
use image::{ImageBuffer, Rgb, RgbImage};
use imageproc::drawing::draw_text_mut;
use ab_glyph::{FontRef, PxScale};

use crate::common::AppError;
use super::{ProcessingResult, traits::ImageGenerator};

const MAX_CHARS_PER_PAGE: usize = 2000;
const IMAGE_WIDTH: u32 = 800;
const IMAGE_HEIGHT: u32 = 600;
const MARGIN: u32 = 20;
const LINE_HEIGHT: f32 = 18.0;
const FONT_SIZE: f32 = 14.0;

pub struct TextImageGenerator;

impl TextImageGenerator {
    /// Resize image to thumbnail size (300px max dimension)
    fn resize_to_thumbnail(image_data: &[u8]) -> Result<Vec<u8>, AppError> {
        const THUMBNAIL_SIZE: u32 = 300;

        // Decode JPEG
        let img = image::load_from_memory(image_data)
            .map_err(|e| AppError::internal_error(format!("Failed to decode image: {}", e)))?;

        // Convert to RGB first (to ensure resize returns RGB)
        let rgb_img = img.to_rgb8();

        // Calculate dimensions maintaining aspect ratio
        let (width, height) = rgb_img.dimensions();
        let (new_width, new_height) = if width > height {
            (THUMBNAIL_SIZE, (height * THUMBNAIL_SIZE) / width)
        } else {
            ((width * THUMBNAIL_SIZE) / height, THUMBNAIL_SIZE)
        };

        // Resize with Lanczos3 filter (high quality) - resize returns an ImageBuffer
        let resized = image::imageops::resize(
            &rgb_img,
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

    /// Detect character encoding and decode text
    fn decode_text(data: &[u8]) -> Result<String, AppError> {
        // Try UTF-8 first (most common)
        if let Ok(content) = String::from_utf8(data.to_vec()) {
            return Ok(content);
        }

        // Try common encodings
        let encodings = [UTF_8, UTF_16LE, UTF_16BE, WINDOWS_1252, ISO_8859_2];

        for encoding in encodings {
            let (content, _, had_errors) = encoding.decode(data);
            if !had_errors {
                return Ok(content.into_owned());
            }
        }

        // Fallback: use UTF-8 with replacement characters
        let (content, _, _) = UTF_8.decode(data);
        Ok(content.into_owned())
    }

    /// Check if file is a code file based on content patterns
    fn is_code_file(mime_type: &str, content: &str) -> bool {
        // Check MIME type
        if matches!(
            mime_type,
            "text/javascript"
                | "application/javascript"
                | "application/json"
                | "text/css"
                | "text/html"
                | "application/xml"
                | "text/xml"
        ) {
            return true;
        }

        // Check content for code patterns
        let code_indicators = ["{", "}", "function", "class", "import", "export", "const", "let", "var"];
        let first_1000_chars = &content.chars().take(1000).collect::<String>();

        code_indicators.iter().filter(|&indicator| first_1000_chars.contains(indicator)).count() >= 3
    }

    /// Wrap text into lines that fit within the image width
    /// Simple character-based wrapping for monospace font
    fn wrap_text(text: &str, max_chars_per_line: usize) -> Vec<String> {
        let mut lines = Vec::new();

        for paragraph in text.lines() {
            if paragraph.is_empty() {
                lines.push(String::new());
                continue;
            }

            // For monospace, we can use simple character counting
            let chars: Vec<char> = paragraph.chars().collect();
            let mut start = 0;

            while start < chars.len() {
                let end = (start + max_chars_per_line).min(chars.len());
                let line: String = chars[start..end].iter().collect();
                lines.push(line);
                start = end;
            }
        }

        lines
    }

    /// Create a text preview image
    fn create_text_image(
        text: &str,
        page_number: u32,
        is_code: bool,
    ) -> Result<Vec<u8>, AppError> {
        // Create white background
        let mut img: RgbImage = ImageBuffer::new(IMAGE_WIDTH, IMAGE_HEIGHT);
        for pixel in img.pixels_mut() {
            *pixel = Rgb([255, 255, 255]);
        }

        // Load embedded DejaVuSansMono font (monospace, good for both code and text)
        let font_data = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/resources/fonts/DejaVuSansMono.ttf"));
        let font = FontRef::try_from_slice(font_data)
            .map_err(|e| AppError::internal_error(format!("Failed to load font: {}", e)))?;

        let scale = PxScale::from(FONT_SIZE);
        let text_color = if is_code {
            Rgb([40, 44, 52]) // Darker for code
        } else {
            Rgb([60, 60, 60]) // Standard dark gray
        };

        // For monospace font, calculate characters per line
        let max_chars_per_line = 90; // ~800px width with 14px font

        // Wrap text to fit in image
        let lines = Self::wrap_text(text, max_chars_per_line);

        // Draw lines
        let mut y_pos = MARGIN as i32;
        let max_lines = ((IMAGE_HEIGHT - 2 * MARGIN) as f32 / LINE_HEIGHT) as usize;

        for line in lines.iter().take(max_lines) {
            if y_pos + LINE_HEIGHT as i32 > (IMAGE_HEIGHT - MARGIN) as i32 {
                break;
            }

            draw_text_mut(
                &mut img,
                text_color,
                MARGIN as i32,
                y_pos,
                scale,
                &font,
                line,
            );

            y_pos += LINE_HEIGHT as i32;
        }

        // Add page indicator if multiple pages
        if page_number > 1 {
            let page_text = format!("Page {}", page_number);
            draw_text_mut(
                &mut img,
                Rgb([150, 150, 150]),
                (IMAGE_WIDTH - 80) as i32,
                (IMAGE_HEIGHT - 30) as i32,
                PxScale::from(12.0),
                &font,
                &page_text,
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

    /// Generate multiple pages if text is long
    fn paginate_text(text: &str) -> Vec<String> {
        let mut pages = Vec::new();
        let lines: Vec<&str> = text.lines().collect();
        let mut current_page = String::new();
        let mut current_chars = 0;

        for line in lines {
            let line_len = line.len() + 1; // +1 for newline

            if current_chars + line_len > MAX_CHARS_PER_PAGE && !current_page.is_empty() {
                pages.push(current_page);
                current_page = String::new();
                current_chars = 0;
            }

            if !current_page.is_empty() {
                current_page.push('\n');
            }
            current_page.push_str(line);
            current_chars += line_len;
        }

        if !current_page.is_empty() {
            pages.push(current_page);
        }

        if pages.is_empty() {
            pages.push(String::new());
        }

        pages
    }
}

#[async_trait]
impl ImageGenerator for TextImageGenerator {
    fn can_generate(&self, mime_type: &str) -> bool {
        matches!(
            mime_type,
            "text/plain"
                | "text/markdown"
                | "text/html"
                | "text/css"
                | "text/javascript"
                | "application/javascript"
                | "application/json"
                | "application/xml"
                | "text/xml"
                | "text/csv"
                | "application/x-sh"
                | "application/x-python"
                | "text/x-rust"
                | "text/x-c"
        )
    }

    async fn generate_images(
        &self,
        data: &[u8],
        mime_type: &str,
        max_thumbnails: u32,
    ) -> Result<ProcessingResult, AppError> {
        // Decode text with encoding detection
        let text = Self::decode_text(data)?;

        // Check if it's code
        let is_code = Self::is_code_file(mime_type, &text);

        // Paginate text
        let pages = Self::paginate_text(&text);
        let num_pages = pages.len().min(max_thumbnails as usize);

        // Generate all preview images at full size
        let mut images = Vec::new();
        for (i, page_text) in pages.iter().take(num_pages).enumerate() {
            let page_num = (i + 1) as u32;
            let image_data = Self::create_text_image(page_text, page_num, is_code)?;
            images.push(image_data);
        }

        // Create single 300px thumbnail from first preview image
        let thumbnails = if let Some(first_image) = images.first() {
            vec![Self::resize_to_thumbnail(first_image)?]
        } else {
            vec![]
        };

        Ok(ProcessingResult {
            text_content: None, // Text extraction is handled by TextProcessor
            metadata: Default::default(),
            thumbnails, // Single element array
            images,     // Multiple elements (one per page)
        })
    }
}
