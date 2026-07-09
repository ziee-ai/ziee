//! whisper-server binary version resolution + readiness.
//!
//! Mirrors `llm_local_runtime::binary_manager` (select_version, check_for_updates,
//! set_system_default, sync_cache), scoped to the single whisper engine. The
//! download + update handlers land in the `runtime_version` layer; this file
//! owns host detection + the readiness check the capability endpoint consumes.

use crate::modules::llm_local_runtime::utils::gpu_detect;

/// Host platform string (`linux` | `macos` | `windows`) — reuses the LLM
/// runtime's detection so the asset-naming contract matches the fork's CI.
pub fn host_platform() -> String {
    gpu_detect::host_platform()
}

/// Host arch string (`x86_64` | `arm64`).
pub fn host_arch() -> String {
    gpu_detect::host_arch()
}

/// True when at least one whisper-server binary is installed for THIS host
/// (any backend) — i.e. the runtime can start. The capability endpoint uses
/// this to decide whether the composer mic is usable.
pub async fn runtime_ready() -> bool {
    let platform = host_platform();
    let arch = host_arch();
    let count = sqlx::query_scalar!(
        r#"SELECT COUNT(*) as "count!" FROM voice_runtime_versions
           WHERE platform = $1 AND arch = $2"#,
        platform,
        arch,
    )
    .fetch_one(crate::core::Repos.pool())
    .await
    .unwrap_or(0);
    count > 0
}
