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
