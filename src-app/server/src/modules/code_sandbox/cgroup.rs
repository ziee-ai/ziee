//! cgroup v2 transient-scope creation.
//!
//! Writes directly to sysfs (no systemd-run — empirically validated:
//! docker containers have no D-Bus session for the user instance, and
//! the chosen delegated parent path is read from
//! `code_sandbox.cgroup_parent` which the deployment is responsible
//! for setting up (systemd `Slice=… Delegate=yes` or docker
//! `cgroup_parent=…` + entrypoint chown).

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use uuid::Uuid;

use crate::common::AppError;

/// Per-call cgroup scope. Cleans up on Drop (best-effort; an already-
/// OOM-killed cgroup auto-cleans when empty so rmdir is harmless).
pub struct CgroupScope {
    path: PathBuf,
}

impl CgroupScope {
    pub fn create(parent: &Path, conversation_id: Uuid) -> Result<Self, AppError> {
        let nanos = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let name = format!("sandbox-{conversation_id}-{nanos}");
        let path = parent.join(&name);
        std::fs::create_dir(&path).map_err(|e| {
            AppError::new(
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "CGROUP_CREATE_FAILED",
                format!("create_dir({}): {e}", path.display()),
            )
        })?;

        // Defaults matched to the rlimits we set via prlimit inside
        // the sandbox. cgroup is defense-in-depth; rlimits do the
        // heavy lifting when cgroup is unavailable.
        //
        // Each write CAN legitimately fail if the parent slice didn't
        // delegate that specific controller (e.g. `cgroup.subtree_control`
        // contains `+memory +pids` but not `+cpu`). The scope still
        // works — it just doesn't enforce the missing controller. We
        // log loudly so operators see the silent quota degradation
        // (without it, the sandbox advertises "512 MiB / 256 PIDs /
        // 1 CPU" while a single missing controller silently means
        // "unlimited within the cgroup" for that resource).
        write_controller(&path, "memory.max", "536870912"); // 512 MiB
        write_controller(&path, "memory.swap.max", "0");
        write_controller(&path, "pids.max", "256");
        write_controller(&path, "cpu.max", "100000 100000"); // 1 CPU

        Ok(Self { path })
    }

    pub fn attach_pid(&self, pid: u32) -> Result<(), AppError> {
        std::fs::write(self.path.join("cgroup.procs"), pid.to_string()).map_err(|e| {
            AppError::new(
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "CGROUP_ATTACH_FAILED",
                format!("write cgroup.procs: {e}"),
            )
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for CgroupScope {
    fn drop(&mut self) {
        // rmdir is best-effort: empty cgroups remove cleanly; non-empty
        // (e.g. zombie left over by SIGKILL race) remove on next sweep.
        let _ = std::fs::remove_dir(&self.path);
    }
}

/// Write a single controller value into the cgroup scope, logging
/// loudly on failure. The scope still functions if a write fails (the
/// other controllers are still active), but the missing controller's
/// quota silently degrades to "unlimited within the cgroup" — which
/// would otherwise contradict the hardening claim the startup log
/// makes ("cgroup_v2: on (delegated)").
fn write_controller(scope: &Path, file: &str, value: &str) {
    let target = scope.join(file);
    if let Err(e) = std::fs::write(&target, value) {
        tracing::warn!(
            controller = file,
            scope = %scope.display(),
            error = %e,
            "code_sandbox: cgroup controller write failed; quota for \
             this controller silently degrades to unlimited within \
             the cgroup. Check the parent slice's cgroup.subtree_control \
             includes +memory +pids +cpu."
        );
    }
}
