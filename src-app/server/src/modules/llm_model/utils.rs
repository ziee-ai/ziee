// LLM Model service layer - validation and business logic
// Following ziee patterns from llm_provider module

use crate::common::r#type::AppError;

use super::types::{CreateLlmModelRequest, UpdateLlmModelRequest};

/// Validate create model request
pub fn validate_create_request(request: &CreateLlmModelRequest) -> Result<(), AppError> {
    // Validate name
    if request.name.trim().is_empty() {
        return Err(AppError::unprocessable_entity(
            "INVALID_NAME",
            "Model name cannot be empty",
        ));
    }

    if request.name.len() > 255 {
        return Err(AppError::unprocessable_entity(
            "INVALID_NAME",
            "Model name cannot exceed 255 characters",
        ));
    }

    // Validate display_name
    if request.display_name.trim().is_empty() {
        return Err(AppError::unprocessable_entity(
            "INVALID_DISPLAY_NAME",
            "Display name cannot be empty",
        ));
    }

    if request.display_name.len() > 255 {
        return Err(AppError::unprocessable_entity(
            "INVALID_DISPLAY_NAME",
            "Display name cannot exceed 255 characters",
        ));
    }

    // Validate parameters if provided
    if let Some(ref params) = request.parameters
        && let Err(e) = params.validate() {
            return Err(AppError::unprocessable_entity("INVALID_PARAMETERS", e));
        }

    // Validate engine settings if provided
    if let Some(ref settings) = request.engine_settings {
        if let Some(ref mistralrs) = settings.mistralrs
            && let Err(e) = mistralrs.validate() {
                return Err(AppError::unprocessable_entity("INVALID_ENGINE_SETTINGS", e));
            }

        if let Some(ref llamacpp) = settings.llamacpp
            && let Err(e) = llamacpp.validate() {
                return Err(AppError::unprocessable_entity("INVALID_ENGINE_SETTINGS", e));
            }
    }

    Ok(())
}

/// Validate update model request
pub fn validate_update_request(request: &UpdateLlmModelRequest) -> Result<(), AppError> {
    // Validate name if provided
    if let Some(ref name) = request.name {
        if name.trim().is_empty() {
            return Err(AppError::unprocessable_entity(
                "INVALID_NAME",
                "Model name cannot be empty",
            ));
        }

        if name.len() > 255 {
            return Err(AppError::unprocessable_entity(
                "INVALID_NAME",
                "Model name cannot exceed 255 characters",
            ));
        }
    }

    // Validate display_name if provided
    if let Some(ref display_name) = request.display_name {
        if display_name.trim().is_empty() {
            return Err(AppError::unprocessable_entity(
                "INVALID_DISPLAY_NAME",
                "Display name cannot be empty",
            ));
        }

        if display_name.len() > 255 {
            return Err(AppError::unprocessable_entity(
                "INVALID_DISPLAY_NAME",
                "Display name cannot exceed 255 characters",
            ));
        }
    }

    // Validate parameters if provided
    if let Some(ref params) = request.parameters
        && let Err(e) = params.validate() {
            return Err(AppError::unprocessable_entity("INVALID_PARAMETERS", e));
        }

    // Validate engine settings if provided
    if let Some(ref settings) = request.engine_settings {
        if let Some(ref mistralrs) = settings.mistralrs
            && let Err(e) = mistralrs.validate() {
                return Err(AppError::unprocessable_entity("INVALID_ENGINE_SETTINGS", e));
            }

        if let Some(ref llamacpp) = settings.llamacpp
            && let Err(e) = llamacpp.validate() {
                return Err(AppError::unprocessable_entity("INVALID_ENGINE_SETTINGS", e));
            }
    }

    Ok(())
}
