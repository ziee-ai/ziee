//! LLM Provider Files Module
//!
//! This module manages the mapping between system files and provider-specific file IDs.
//! It enables cost optimization by caching file uploads to provider Files APIs
//! (Anthropic Files API, Gemini File API) and reusing them across multiple messages.
//!
//! # Features
//! - File upload caching and reuse
//! - Automatic expiration handling (Gemini 48h TTL)
//! - API key rotation support (test-and-validate approach)
//! - Background cleanup jobs
//!
//! # Usage
//! ```no_run
//! use llm_provider_files::service;
//!
//! // Get or upload file to provider
//! let provider_file_id = service::get_or_upload_provider_file(
//!     &pool,
//!     &file_storage,
//!     file_id,
//!     &provider,
//!     &ai_provider,
//! ).await?;
//! ```

pub mod models;
pub mod repository;
pub mod service;

pub use models::*;
