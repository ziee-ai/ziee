//! The `SandboxMountProvider` the desktop registers against the generic server
//! seam. Given a sandbox execution context it resolves the effective host-folder
//! mounts (read-through), enforces the deployment policy, derives the
//! deterministic `/mnt/<full host path>` target, and hands `MountSpec`s back to
//! `code_sandbox`. The server core never learns this is about "host folders".

use std::path::PathBuf;

use sqlx::PgPool;
use ziee::async_trait;
use ziee::code_sandbox::{MountSpec, SandboxContext, SandboxMountProvider, StageMode};
use ziee::AppError;

use super::paths::derive_sandbox_path;
use super::repository::HostMountRepository;

pub struct DesktopHostMountProvider {
    pool: PgPool,
}

impl DesktopHostMountProvider {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl SandboxMountProvider for DesktopHostMountProvider {
    async fn mounts_for(&self, ctx: &SandboxContext) -> Result<Vec<MountSpec>, AppError> {
        let repo = HostMountRepository::new(self.pool.clone());

        let policy = repo.get_policy().await?;
        if !policy.enabled {
            return Ok(Vec::new());
        }

        let entries = repo
            .resolve_effective(ctx.conversation_id, ctx.user_id)
            .await?;

        let mut specs = Vec::new();
        for e in entries {
            // Prefix allowlist (empty = allow any path — the single-user case).
            if !policy.allowed_prefixes.is_empty()
                && !policy
                    .allowed_prefixes
                    .iter()
                    .any(|p| e.host_path.starts_with(p.as_str()))
            {
                continue;
            }
            // Read-write is opt-in at BOTH the per-mount flag and the policy.
            let read_only = e.read_only || !policy.allow_readwrite;
            specs.push(MountSpec {
                mode: if read_only {
                    StageMode::ReadOnly
                } else {
                    StageMode::ReadWrite
                },
                host_path: PathBuf::from(&e.host_path),
                sandbox_path: derive_sandbox_path(&e.host_path),
            });
        }
        // Missing/unreachable sources + protected targets are dropped (with a
        // note to the model) by the server-side generic guard in
        // `code_sandbox::mount_provider`.
        Ok(specs)
    }
}
