//! Runtime-configurable resource limits for the code sandbox (Plan 1 §6).
//!
//! Singleton row in `code_sandbox_settings` (see migration 41). At runtime,
//! [`CodeSandboxState`] caches the row; PUT-via-handler invalidates the cache.
//! The hot path reads the cached snapshot and passes the values into:
//!   - [`crate::modules::code_sandbox::cgroup::CgroupScope`] (Linux host) —
//!     `memory.max`, `pids.max`, `cpu.max`, `memory.swap.max`.
//!   - [`crate::modules::code_sandbox::sandbox::build_bwrap_argv`] prlimit
//!     literals — `--as`, `--fsize`, `--nproc`, `--nofile`, `--cpu`.
//!   - [`sandbox_vm_protocol::CgroupLimits`] on `ExecRequest` for the
//!     macOS / WSL2 backends — the agent applies the same policy in-guest.
//!   - `DEFAULT_TIMEOUT_SECS` / `VM_IDLE_EVICT_SECS` constants are now read
//!     from the settings instead of compile-time consts.
//!
//! Defaults match the prior hardcoded values so existing tests + operator
//! intuition are unchanged on a fresh install.

use axum::http::StatusCode;
use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::common::AppError;
use crate::modules::code_sandbox::repository::CodeSandboxRepository;

/// One row of `code_sandbox_settings`. Field order mirrors the SQL column
/// order; field names match the SQL column names (snake_case) so the sqlx
/// `query_as` mapping is trivial.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, sqlx::FromRow)]
pub struct CodeSandboxResourceLimits {
    /// cgroup v2 `memory.max`.
    pub memory_max_bytes: i64,
    /// cgroup v2 `memory.swap.max`. `0` disables swap.
    pub memory_swap_max_bytes: i64,
    /// cgroup v2 `pids.max`.
    pub pids_max: i32,
    /// cgroup v2 `cpu.max`. `"<quota> <period>"` in microseconds.
    pub cpu_max: String,

    /// `prlimit --as` (virtual address space).
    pub address_space_bytes: i64,
    /// `prlimit --fsize` (single-file max size).
    pub fsize_bytes: i64,
    /// `prlimit --nproc`.
    pub nproc_max: i32,
    /// `prlimit --nofile`.
    pub nofile_max: i32,
    /// `prlimit --cpu` (CPU-seconds backstop).
    pub cpu_secs_max: i32,

    /// Wall-clock per-`execute_command` budget.
    pub timeout_secs: i32,

    /// VM idle eviction (macOS libkrun + WSL2 distro). `0` = never.
    pub vm_idle_evict_secs: i32,

    /// macOS libkrun microVM vCPU count (`krun_set_vm_config`).
    pub mac_vm_vcpus: i32,
    /// macOS libkrun microVM RAM ceiling in MiB.
    pub mac_vm_ram_mib: i32,
    /// Per-VM concurrent `execute_command` cap (macOS + WSL2).
    pub vm_max_concurrent_execs: i32,

    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Admin PUT payload. All fields optional; absent fields preserve their
/// existing value. Mirrors the `llm_provider` admin-update pattern.
#[derive(Debug, Clone, Default, Deserialize, JsonSchema)]
pub struct UpdateCodeSandboxResourceLimits {
    pub memory_max_bytes: Option<i64>,
    pub memory_swap_max_bytes: Option<i64>,
    pub pids_max: Option<i32>,
    pub cpu_max: Option<String>,

    pub address_space_bytes: Option<i64>,
    pub fsize_bytes: Option<i64>,
    pub nproc_max: Option<i32>,
    pub nofile_max: Option<i32>,
    pub cpu_secs_max: Option<i32>,

    pub timeout_secs: Option<i32>,
    pub vm_idle_evict_secs: Option<i32>,
    pub mac_vm_vcpus: Option<i32>,
    pub mac_vm_ram_mib: Option<i32>,
    pub vm_max_concurrent_execs: Option<i32>,
}

impl UpdateCodeSandboxResourceLimits {
    /// Validate ranges + clamp before any DB write. The DB has constraints as
    /// a backstop, but a structured error here returns a clearer 422 to the
    /// admin UI than a Postgres CHECK violation would. Bounds mirror the
    /// SQL `CHECK` constraints in migration 41.
    pub fn validate(&self) -> Result<(), AppError> {
        fn bad(field: &str, msg: impl Into<String>) -> AppError {
            AppError::new(
                StatusCode::UNPROCESSABLE_ENTITY,
                "SANDBOX_LIMIT_OUT_OF_RANGE",
                format!("invalid {field}: {}", msg.into()),
            )
        }
        if let Some(v) = self.memory_max_bytes {
            if v < 16 * 1024 * 1024 {
                return Err(bad("memory_max_bytes", "must be ≥ 16 MiB"));
            }
        }
        if let Some(v) = self.memory_swap_max_bytes {
            if v < 0 {
                return Err(bad("memory_swap_max_bytes", "must be ≥ 0"));
            }
        }
        if let Some(v) = self.pids_max {
            if !(8..=100_000).contains(&v) {
                return Err(bad("pids_max", "must be in 8..=100000"));
            }
        }
        if let Some(v) = self.address_space_bytes {
            if v < 16 * 1024 * 1024 {
                return Err(bad("address_space_bytes", "must be ≥ 16 MiB"));
            }
        }
        if let Some(v) = self.fsize_bytes {
            if v < 1024 * 1024 {
                return Err(bad("fsize_bytes", "must be ≥ 1 MiB"));
            }
        }
        if let Some(v) = self.nproc_max {
            if !(8..=100_000).contains(&v) {
                return Err(bad("nproc_max", "must be in 8..=100000"));
            }
        }
        if let Some(v) = self.nofile_max {
            if !(64..=1_048_576).contains(&v) {
                return Err(bad("nofile_max", "must be in 64..=1048576"));
            }
        }
        if let Some(v) = self.cpu_secs_max {
            if !(10..=86_400).contains(&v) {
                return Err(bad("cpu_secs_max", "must be in 10..=86400"));
            }
        }
        if let Some(v) = self.timeout_secs {
            if !(5..=86_400).contains(&v) {
                return Err(bad("timeout_secs", "must be in 5..=86400"));
            }
        }
        if let Some(v) = self.vm_idle_evict_secs {
            if v < 0 {
                return Err(bad("vm_idle_evict_secs", "must be ≥ 0"));
            }
        }
        if let Some(s) = self.cpu_max.as_deref() {
            // Stricter than the DB's `~ '^[0-9]+ [0-9]+$'` regex: require both
            // values to be parseable u64 + non-zero.
            let parts: Vec<&str> = s.split(' ').collect();
            if parts.len() != 2 {
                return Err(bad("cpu_max", "want \"<quota_us> <period_us>\""));
            }
            let q: u64 = parts[0]
                .parse()
                .map_err(|_| bad("cpu_max", "quota is not a non-negative integer"))?;
            let p: u64 = parts[1]
                .parse()
                .map_err(|_| bad("cpu_max", "period is not a non-negative integer"))?;
            if p == 0 {
                return Err(bad("cpu_max", "period must be > 0"));
            }
            // Reject anything that would amount to less than 1% of a CPU — a
            // typo here would deadlock every job.
            if q != 0 && q < p / 100 {
                return Err(bad(
                    "cpu_max",
                    "quota < 1% of period (would starve every job)",
                ));
            }
        }
        if let Some(v) = self.mac_vm_vcpus {
            if !(1..=128).contains(&v) {
                return Err(bad("mac_vm_vcpus", "must be in 1..=128"));
            }
        }
        if let Some(v) = self.mac_vm_ram_mib {
            if !(256..=262_144).contains(&v) {
                return Err(bad("mac_vm_ram_mib", "must be in 256..=262144 (MiB)"));
            }
        }
        if let Some(v) = self.vm_max_concurrent_execs {
            if !(1..=1000).contains(&v) {
                return Err(bad("vm_max_concurrent_execs", "must be in 1..=1000"));
            }
        }
        Ok(())
    }
}

impl CodeSandboxRepository {
    /// Read the singleton settings row. Always returns Some because the
    /// migration seeds it; a missing row would mean the DB is partially
    /// migrated, in which case bubble up an internal error.
    pub async fn get_resource_limits(
        &self,
    ) -> Result<CodeSandboxResourceLimits, AppError> {
        let row: Option<CodeSandboxResourceLimits> = sqlx::query_as(
            r#"
            SELECT memory_max_bytes, memory_swap_max_bytes, pids_max, cpu_max,
                   address_space_bytes, fsize_bytes, nproc_max, nofile_max, cpu_secs_max,
                   timeout_secs, vm_idle_evict_secs,
                   mac_vm_vcpus, mac_vm_ram_mib, vm_max_concurrent_execs,
                   created_at, updated_at
            FROM code_sandbox_settings
            WHERE id = TRUE
            "#,
        )
        .fetch_optional(self.pool())
        .await
        .map_err(|e| {
            tracing::error!(error = ?e, "code_sandbox: read resource_limits");
            AppError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "DATABASE_ERROR",
                "database error",
            )
        })?;
        row.ok_or_else(|| {
            AppError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "SANDBOX_SETTINGS_MISSING",
                "code_sandbox_settings singleton row is missing — run migrations",
            )
        })
    }

    /// PATCH-style update: only the `Some` fields are written. Returns the
    /// post-update row so the handler can return it without a second SELECT.
    /// Validation happens at the call site (see
    /// [`UpdateCodeSandboxResourceLimits::validate`]).
    pub async fn update_resource_limits(
        &self,
        patch: &UpdateCodeSandboxResourceLimits,
    ) -> Result<CodeSandboxResourceLimits, AppError> {
        let row: Option<CodeSandboxResourceLimits> = sqlx::query_as(
            r#"
            UPDATE code_sandbox_settings SET
                memory_max_bytes        = COALESCE($1, memory_max_bytes),
                memory_swap_max_bytes   = COALESCE($2, memory_swap_max_bytes),
                pids_max                = COALESCE($3, pids_max),
                cpu_max                 = COALESCE($4, cpu_max),
                address_space_bytes     = COALESCE($5, address_space_bytes),
                fsize_bytes             = COALESCE($6, fsize_bytes),
                nproc_max               = COALESCE($7, nproc_max),
                nofile_max              = COALESCE($8, nofile_max),
                cpu_secs_max            = COALESCE($9, cpu_secs_max),
                timeout_secs            = COALESCE($10, timeout_secs),
                vm_idle_evict_secs      = COALESCE($11, vm_idle_evict_secs),
                mac_vm_vcpus            = COALESCE($12, mac_vm_vcpus),
                mac_vm_ram_mib          = COALESCE($13, mac_vm_ram_mib),
                vm_max_concurrent_execs = COALESCE($14, vm_max_concurrent_execs),
                updated_at              = NOW()
            WHERE id = TRUE
            RETURNING memory_max_bytes, memory_swap_max_bytes, pids_max, cpu_max,
                      address_space_bytes, fsize_bytes, nproc_max, nofile_max, cpu_secs_max,
                      timeout_secs, vm_idle_evict_secs,
                      mac_vm_vcpus, mac_vm_ram_mib, vm_max_concurrent_execs,
                      created_at, updated_at
            "#,
        )
        .bind(patch.memory_max_bytes)
        .bind(patch.memory_swap_max_bytes)
        .bind(patch.pids_max)
        .bind(patch.cpu_max.as_deref())
        .bind(patch.address_space_bytes)
        .bind(patch.fsize_bytes)
        .bind(patch.nproc_max)
        .bind(patch.nofile_max)
        .bind(patch.cpu_secs_max)
        .bind(patch.timeout_secs)
        .bind(patch.vm_idle_evict_secs)
        .bind(patch.mac_vm_vcpus)
        .bind(patch.mac_vm_ram_mib)
        .bind(patch.vm_max_concurrent_execs)
        .fetch_optional(self.pool())
        .await
        .map_err(|e| {
            tracing::error!(error = ?e, "code_sandbox: update resource_limits");
            // Surface the most common operator mistake (CHECK constraint
            // violation) with a 422 so the UI can render it clearly.
            if let sqlx::Error::Database(db) = &e {
                if db.constraint().is_some() {
                    return AppError::new(
                        StatusCode::UNPROCESSABLE_ENTITY,
                        "SANDBOX_LIMIT_DB_CONSTRAINT",
                        format!(
                            "value rejected by DB constraint {:?}: {}",
                            db.constraint(),
                            db.message()
                        ),
                    );
                }
            }
            AppError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "DATABASE_ERROR",
                "database error",
            )
        })?;
        row.ok_or_else(|| {
            AppError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "SANDBOX_SETTINGS_MISSING",
                "code_sandbox_settings singleton row is missing — run migrations",
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok(patch: UpdateCodeSandboxResourceLimits) -> bool {
        patch.validate().is_ok()
    }
    fn err(patch: UpdateCodeSandboxResourceLimits) -> bool {
        patch.validate().is_err()
    }

    #[test]
    fn empty_patch_validates() {
        assert!(ok(UpdateCodeSandboxResourceLimits::default()));
    }

    #[test]
    fn memory_lower_bound() {
        assert!(err(UpdateCodeSandboxResourceLimits {
            memory_max_bytes: Some(16 * 1024 * 1024 - 1),
            ..Default::default()
        }));
        assert!(ok(UpdateCodeSandboxResourceLimits {
            memory_max_bytes: Some(16 * 1024 * 1024),
            ..Default::default()
        }));
    }

    #[test]
    fn cpu_max_shape() {
        for bad in ["", "100000", "100000 ", "abc 100000", "100000 0", "100000 abc"] {
            assert!(
                err(UpdateCodeSandboxResourceLimits {
                    cpu_max: Some(bad.to_string()),
                    ..Default::default()
                }),
                "expected {bad:?} to fail"
            );
        }
        for good in ["100000 100000", "50000 100000", "0 100000"] {
            assert!(
                ok(UpdateCodeSandboxResourceLimits {
                    cpu_max: Some(good.to_string()),
                    ..Default::default()
                }),
                "expected {good:?} to pass"
            );
        }
    }

    #[test]
    fn cpu_max_starvation_guard() {
        // 1 µs quota in 100 ms period = 0.001% of a CPU; reject.
        assert!(err(UpdateCodeSandboxResourceLimits {
            cpu_max: Some("1 100000".to_string()),
            ..Default::default()
        }));
        // 1% boundary should pass.
        assert!(ok(UpdateCodeSandboxResourceLimits {
            cpu_max: Some("1000 100000".to_string()),
            ..Default::default()
        }));
    }

    #[test]
    fn ranges_at_boundaries() {
        assert!(ok(UpdateCodeSandboxResourceLimits {
            pids_max: Some(8),
            ..Default::default()
        }));
        assert!(err(UpdateCodeSandboxResourceLimits {
            pids_max: Some(7),
            ..Default::default()
        }));
        assert!(ok(UpdateCodeSandboxResourceLimits {
            pids_max: Some(100_000),
            ..Default::default()
        }));
        assert!(err(UpdateCodeSandboxResourceLimits {
            pids_max: Some(100_001),
            ..Default::default()
        }));
    }
}
