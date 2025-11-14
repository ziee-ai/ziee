// Image file processor

use super::traits::ImageGenerator;
use super::ProcessingResult;
use crate::common::AppError;
use crate::modules::file::models::ProcessingMetadata;
use async_trait::async_trait;
use image::{imageops::FilterType, DynamicImage, GenericImageView, ImageFormat};
use std::io::Cursor;

/// Image processor
pub struct ImageProcessor;

impl ImageProcessor {
    /// Resize image maintaining aspect ratio
    fn resize_image(img: &DynamicImage, max_dimension: u32) -> DynamicImage {
        let (width, height) = img.dimensions();

        if width <= max_dimension && height <= max_dimension {
            return img.clone();
        }

        let (new_width, new_height) = if width > height {
            let ratio = max_dimension as f32 / width as f32;
            (max_dimension, (height as f32 * ratio) as u32)
        } else {
            let ratio = max_dimension as f32 / height as f32;
            ((width as f32 * ratio) as u32, max_dimension)
        };

        img.resize(new_width, new_height, FilterType::Lanczos3)
    }

    /// Encode image to JPEG bytes
    fn encode_jpeg(img: &DynamicImage) -> Result<Vec<u8>, AppError> {
        let mut buffer = Vec::new();
        let mut cursor = Cursor::new(&mut buffer);

        img.write_to(&mut cursor, ImageFormat::Jpeg)
            .map_err(|e| AppError::internal_error(format!("Failed to encode JPEG: {}", e)))?;

        Ok(buffer)
    }
}

#[async_trait]
impl ImageGenerator for ImageProcessor {
    fn can_generate(&self, mime_type: &str) -> bool {
        matches!(
            mime_type,
            "image/jpeg" | "image/jpg" | "image/png" | "image/gif"
                | "image/webp" | "image/bmp" | "image/tiff"
        )
    }

    async fn generate_images(
        &self,
        data: &[u8],
        mime_type: &str,
        _max_thumbnails: u32,
    ) -> Result<ProcessingResult, AppError> {
        // Load image
        let img = image::load_from_memory(data)
            .map_err(|e| AppError::internal_error(format!("Failed to load image: {}", e)))?;

        let (width, height) = img.dimensions();

        // Generate single high-quality image (max 2000px)
        let high_quality = Self::resize_image(&img, 2000);
        let high_quality_bytes = Self::encode_jpeg(&high_quality)?;

        // Generate single thumbnail (max 300px)
        let thumbnail = Self::resize_image(&img, 300);
        let thumbnail_bytes = Self::encode_jpeg(&thumbnail)?;

        let metadata = ProcessingMetadata {
            width: Some(width),
            height: Some(height),
            format: Some(mime_type.to_string()),
            has_text: Some(false),
            ..Default::default()
        };

        Ok(ProcessingResult {
            text_content: None,
            metadata,
            thumbnails: vec![thumbnail_bytes],
            images: vec![high_quality_bytes],
        })
    }
}
