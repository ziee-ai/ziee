use std::path::PathBuf;
use std::time::Duration;
use tokio::process::Command;
use tokio::time::timeout;
use uuid::Uuid;

use crate::common::AppError;
use crate::modules::code_sandbox::models::ConversationFile;

pub struct SandboxConfig {
    pub data_dir: PathBuf,
    pub rootfs_path: PathBuf,
    pub conversation_id: Uuid,
    pub user_id: Uuid,
    pub files: Vec<ConversationFile>,
}

pub struct SandboxOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

impl SandboxConfig {
    pub fn workspace_path(&self) -> PathBuf {
        self.data_dir
            .join("sandboxes")
            .join(self.conversation_id.to_string())
    }

    pub fn originals_path(&self) -> PathBuf {
        self.data_dir.join("files").join("originals")
    }
}

/// Run a shell command inside the bwrap sandbox.
pub async fn run_in_sandbox(
    config: &SandboxConfig,
    command: &str,
    exec_timeout: Duration,
) -> Result<SandboxOutput, AppError> {
    let workspace = config.workspace_path();
    tokio::fs::create_dir_all(&workspace).await.map_err(|e| {
        AppError::internal_error(format!("Failed to create workspace: {}", e))
    })?;

    let rootfs = &config.rootfs_path;

    // Build the bwrap command
    let mut cmd = Command::new("bwrap");

    cmd.args([
        "--unshare-user",
        "--uid", "1001",
        "--gid", "1001",
    ]);

    // Mount rootfs /usr read-only and create virtual symlinks for /bin, /lib, /lib64
    cmd.args([
        "--ro-bind", &format!("{}/usr", rootfs.display()), "/usr",
        "--symlink", "usr/bin", "/bin",
        "--symlink", "usr/lib", "/lib",
        "--symlink", "usr/lib64", "/lib64",
    ]);

    // Mount rootfs /etc (covers ssl, R, alternatives, and other system config).
    // The synthesized /etc/passwd and /etc/group mounts below override these specific files.
    let etc_path = rootfs.join("etc");
    if etc_path.exists() {
        cmd.args([
            "--ro-bind", &etc_path.to_string_lossy(), "/etc",
        ]);
    }

    // Standard mounts
    cmd.args([
        "--proc", "/proc",
        "--dev", "/dev",
        "--tmpfs", "/tmp",
    ]);

    // Inject /etc/passwd and /etc/group via file descriptors using a temp file approach.
    // We write them to a temp location in the workspace and bind-mount them.
    let passwd_content = "sandboxuser:x:1001:1001::/home/sandboxuser:/bin/bash\n";
    let group_content = "sandboxuser:x:1001:\n";
    let passwd_path = workspace.join(".sandbox_passwd");
    let group_path = workspace.join(".sandbox_group");
    tokio::fs::write(&passwd_path, passwd_content).await.map_err(|e| {
        AppError::internal_error(format!("Failed to write passwd: {}", e))
    })?;
    tokio::fs::write(&group_path, group_content).await.map_err(|e| {
        AppError::internal_error(format!("Failed to write group: {}", e))
    })?;
    cmd.args([
        "--ro-bind", &passwd_path.to_string_lossy(), "/etc/passwd",
        "--ro-bind", &group_path.to_string_lossy(), "/etc/group",
    ]);

    // Bind workspace as home directory (writable — persists across runs within same conversation)
    cmd.args([
        "--bind", &workspace.to_string_lossy(), "/home/sandboxuser",
        "--chdir", "/home/sandboxuser",
    ]);

    // Mount user-uploaded files from the conversation as read-only.
    // Files are stored on disk as {file_id}.{ext} but exposed inside the sandbox by their original filename.
    let originals = config.originals_path();
    for file in &config.files {
        let ext = file.mime_type
            .as_deref()
            .and_then(|m| mime_to_ext(m))
            .unwrap_or("bin");
        let src = originals
            .join(file.user_id.to_string())
            .join(format!("{}.{}", file.file_id, ext));
        if src.exists() {
            let dst = format!("/home/sandboxuser/{}", file.filename);
            cmd.args(["--ro-bind", &src.to_string_lossy(), &dst]);
        }
    }

    // New session to prevent signal propagation
    cmd.arg("--new-session");

    // Shell command to execute
    cmd.args(["/bin/sh", "-c", command]);

    // Capture stdout and stderr
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());

    let run = async {
        let child = cmd.spawn().map_err(|e| {
            AppError::internal_error(format!("Failed to spawn bwrap: {}", e))
        })?;

        let output = child.wait_with_output().await.map_err(|e| {
            AppError::internal_error(format!("bwrap process error: {}", e))
        })?;

        let exit_code = output.status.code().unwrap_or(-1);
        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();

        Ok::<SandboxOutput, AppError>(SandboxOutput {
            stdout,
            stderr,
            exit_code,
        })
    };

    match timeout(exec_timeout, run).await {
        Ok(result) => result,
        Err(_) => Err(AppError::internal_error(
            "Command timed out (10 minute limit exceeded)",
        )),
    }
}

/// Map common MIME types to file extensions for finding files on disk.
pub fn mime_to_ext(mime: &str) -> Option<&'static str> {
    match mime {
        "text/plain" => Some("txt"),
        "text/csv" => Some("csv"),
        "text/html" => Some("html"),
        "text/markdown" => Some("md"),
        "application/json" => Some("json"),
        "application/pdf" => Some("pdf"),
        "application/zip" => Some("zip"),
        "application/x-tar" => Some("tar"),
        "application/gzip" | "application/x-gzip" => Some("gz"),
        "image/png" => Some("png"),
        "image/jpeg" => Some("jpg"),
        "image/gif" => Some("gif"),
        "image/webp" => Some("webp"),
        "image/svg+xml" => Some("svg"),
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet" => Some("xlsx"),
        "application/vnd.ms-excel" => Some("xls"),
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document" => Some("docx"),
        "application/msword" => Some("doc"),
        "audio/mpeg" => Some("mp3"),
        "audio/wav" => Some("wav"),
        "video/mp4" => Some("mp4"),
        "application/octet-stream" => Some("bin"),
        _ => {
            // Try extracting from MIME like "text/x-python" → not matching, return None
            None
        }
    }
}
