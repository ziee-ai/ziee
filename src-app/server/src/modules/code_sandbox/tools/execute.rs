use std::path::PathBuf;
use std::time::Duration;
use uuid::Uuid;

use crate::common::AppError;
use crate::modules::code_sandbox::models::ConversationFile;
use crate::modules::code_sandbox::sandbox::{run_in_sandbox, SandboxConfig};

pub struct ExecuteArgs {
    pub command: String,
}

pub struct ExecuteOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

pub async fn execute_command(
    data_dir: &PathBuf,
    rootfs_path: &PathBuf,
    conversation_id: Uuid,
    user_id: Uuid,
    files: Vec<ConversationFile>,
    args: ExecuteArgs,
) -> Result<ExecuteOutput, AppError> {
    let config = SandboxConfig {
        data_dir: data_dir.clone(),
        rootfs_path: rootfs_path.clone(),
        conversation_id,
        user_id,
        files,
    };

    let output = run_in_sandbox(&config, &args.command, Duration::from_secs(600)).await?;

    Ok(ExecuteOutput {
        stdout: output.stdout,
        stderr: output.stderr,
        exit_code: output.exit_code,
    })
}
