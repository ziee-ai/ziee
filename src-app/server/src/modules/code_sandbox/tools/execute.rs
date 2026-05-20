//! `execute_command` — invokes bwrap with a user-supplied shell command.
//! Body lands in Phase 4.

use crate::common::AppError;
use crate::modules::code_sandbox::types::SandboxContext;

pub async fn execute_command(
    _ctx: &SandboxContext,
    _command: &str,
) -> Result<serde_json::Value, AppError> {
    Err(AppError::new(
        axum::http::StatusCode::NOT_IMPLEMENTED,
        "NOT_IMPLEMENTED",
        "execute_command not yet implemented",
    ))
}
