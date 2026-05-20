//! Filesystem tools: read_file, write_file, edit_file, list_files,
//! get_resource_link. Bodies land in Phase 4.

use crate::common::AppError;
use crate::modules::code_sandbox::types::SandboxContext;

fn not_yet_impl(name: &str) -> AppError {
    AppError::new(
        axum::http::StatusCode::NOT_IMPLEMENTED,
        "NOT_IMPLEMENTED",
        format!("{name} not yet implemented"),
    )
}

pub async fn read_file(
    _ctx: &SandboxContext,
    _filename: &str,
    _start_line: Option<usize>,
    _end_line: Option<usize>,
) -> Result<serde_json::Value, AppError> {
    Err(not_yet_impl("read_file"))
}

pub async fn write_file(
    _ctx: &SandboxContext,
    _filename: &str,
    _content: &str,
) -> Result<serde_json::Value, AppError> {
    Err(not_yet_impl("write_file"))
}

pub async fn edit_file(
    _ctx: &SandboxContext,
    _filename: &str,
    _start_line: usize,
    _end_line: usize,
    _new_content: &str,
) -> Result<serde_json::Value, AppError> {
    Err(not_yet_impl("edit_file"))
}

pub async fn list_files(_ctx: &SandboxContext) -> Result<serde_json::Value, AppError> {
    Err(not_yet_impl("list_files"))
}

pub async fn get_resource_link(
    _ctx: &SandboxContext,
    _filename: &str,
    _save_as: Option<&str>,
) -> Result<serde_json::Value, AppError> {
    Err(not_yet_impl("get_resource_link"))
}
