//! One-time boot probes for the sandbox.
//!
//! Probes run exactly once at `code_sandbox::init()` and the results
//! land in `HardeningCapabilities`. Every per-call code path reads
//! that cached struct — no shellouts on the hot path.

use std::path::{Path, PathBuf};
use std::process::Command as StdCommand;

use crate::core::config::CodeSandboxConfig;
use crate::modules::code_sandbox::types::{
    CgroupMode, HardeningCapabilities, PidNsMode, SeccompMode,
};

/// Run all probes sequentially. Cost: ~50-100 ms one-time.
pub fn probe_all(config: &CodeSandboxConfig) -> HardeningCapabilities {
    let bwrap_path = which_bwrap().unwrap_or_else(|| PathBuf::from("bwrap"));
    let pid_namespace = probe_pid_ns(&bwrap_path, &config.rootfs_path);
    let cgroup = probe_cgroup(&config.cgroup_parent);
    let seccomp = compile_seccomp_filter();

    let summary = format!(
        "code_sandbox: hardening = {{ rlimits: on, bwrap: {bwrap}, pid_ns: {pid_ns}, cgroup_v2: {cg}, seccomp: {sc} }}",
        bwrap = if bwrap_path.is_absolute() { "on" } else { "MISSING" },
        pid_ns = match pid_namespace {
            PidNsMode::Strict => "on",
            PidNsMode::DevBindFallback => "off-fallback-dev-bind",
            PidNsMode::Disabled => "DISABLED",
        },
        cg = match &cgroup {
            CgroupMode::Delegated(_) => "on (delegated)",
            CgroupMode::None => "off-needs-delegation",
        },
        sc = match &seccomp {
            SeccompMode::Loaded(_) => "on",
            SeccompMode::NotLinked => "off-feature-not-linked",
            SeccompMode::Disabled => "off-libseccomp-failed",
        },
    );
    tracing::info!("{summary}");

    HardeningCapabilities {
        bwrap_path,
        pid_namespace,
        cgroup,
        seccomp,
    }
}

fn which_bwrap() -> Option<PathBuf> {
    for dir in std::env::var_os("PATH")?.to_string_lossy().split(':') {
        let p = Path::new(dir).join("bwrap");
        if p.is_file() {
            return Some(p);
        }
    }
    None
}

/// Try strict mode (`--unshare-pid --proc /proc`) first; on failure
/// retry with `--dev-bind /proc /proc`. If neither works, sandbox is
/// disabled.
fn probe_pid_ns(bwrap_path: &Path, rootfs: &str) -> PidNsMode {
    if !bwrap_path.is_absolute() {
        return PidNsMode::Disabled;
    }
    let rootfs_usr = format!("{rootfs}/usr");
    if !Path::new(&rootfs_usr).exists() {
        // Rootfs not mounted; we can't run a probe. Disable gracefully.
        tracing::warn!(
            "code_sandbox: rootfs not present at {rootfs}; PID-ns probe skipped, sandbox disabled"
        );
        return PidNsMode::Disabled;
    }

    let strict = StdCommand::new(bwrap_path)
        .args([
            "--unshare-user",
            "--uid",
            "1001",
            "--gid",
            "1001",
            "--unshare-pid",
            "--share-net",
            "--new-session",
            "--die-with-parent",
            "--ro-bind",
            &rootfs_usr,
            "/usr",
            "--symlink",
            "usr/bin",
            "/bin",
            "--symlink",
            "usr/lib",
            "/lib",
            "--symlink",
            "usr/lib64",
            "/lib64",
            "--proc",
            "/proc",
            "--dev",
            "/dev",
            "--tmpfs",
            "/tmp",
            "--",
            "/bin/true",
        ])
        .output();

    if let Ok(o) = strict {
        if o.status.success() {
            return PidNsMode::Strict;
        }
    }

    // Fallback: same flags but bind /proc.
    let fallback = StdCommand::new(bwrap_path)
        .args([
            "--unshare-user",
            "--uid",
            "1001",
            "--gid",
            "1001",
            "--share-net",
            "--new-session",
            "--die-with-parent",
            "--ro-bind",
            &rootfs_usr,
            "/usr",
            "--symlink",
            "usr/bin",
            "/bin",
            "--symlink",
            "usr/lib",
            "/lib",
            "--symlink",
            "usr/lib64",
            "/lib64",
            "--dev-bind",
            "/proc",
            "/proc",
            "--dev",
            "/dev",
            "--tmpfs",
            "/tmp",
            "--",
            "/bin/true",
        ])
        .output();

    match fallback {
        Ok(o) if o.status.success() => PidNsMode::DevBindFallback,
        _ => PidNsMode::Disabled,
    }
}

/// Detect a writable delegated cgroup parent. Empty config → None.
fn probe_cgroup(parent_str: &str) -> CgroupMode {
    if parent_str.trim().is_empty() {
        return CgroupMode::None;
    }
    let parent = PathBuf::from(parent_str);
    let subtree = parent.join("cgroup.subtree_control");
    if !subtree.exists() {
        tracing::warn!(
            "code_sandbox: cgroup parent {} missing or not delegated; rlimits-only mode",
            parent.display()
        );
        return CgroupMode::None;
    }
    // Read subtree_control: must contain at least `memory` and `pids`
    // for our scope-setting to work.
    let controllers = std::fs::read_to_string(&subtree).unwrap_or_default();
    if !controllers.contains("memory") || !controllers.contains("pids") {
        tracing::warn!(
            "code_sandbox: cgroup parent {} subtree_control lacks memory+pids ({controllers:?}); rlimits-only mode",
            parent.display()
        );
        return CgroupMode::None;
    }
    // Sanity: can we mkdir + rmdir a probe child?
    let probe = parent.join(".sandbox-probe");
    let _ = std::fs::remove_dir(&probe); // ignore prior leftovers
    if std::fs::create_dir(&probe).is_ok() {
        let _ = std::fs::remove_dir(&probe);
        CgroupMode::Delegated(parent)
    } else {
        tracing::warn!(
            "code_sandbox: cgroup parent {} not writable by server uid; rlimits-only mode",
            parent.display()
        );
        CgroupMode::None
    }
}

/// Compile the seccomp filter once at boot. Gated on the
/// `code_sandbox_seccomp` cargo feature; when off, returns
/// `SeccompMode::NotLinked`.
fn compile_seccomp_filter() -> SeccompMode {
    #[cfg(not(feature = "code_sandbox_seccomp"))]
    {
        SeccompMode::NotLinked
    }
    #[cfg(feature = "code_sandbox_seccomp")]
    {
        match seccomp_impl::build() {
            Ok(bytes) => SeccompMode::Loaded(std::sync::Arc::new(bytes)),
            Err(e) => {
                tracing::warn!("code_sandbox: seccomp filter build failed: {e}");
                SeccompMode::Disabled
            }
        }
    }
}

#[cfg(feature = "code_sandbox_seccomp")]
mod seccomp_impl {
    use libseccomp::{ScmpAction, ScmpFilterContext, ScmpSyscall};

    /// Syscalls we deny (return EPERM). Matches Flatpak / Anthropic
    /// Claude Code baseline; see plan Phase 3.
    const DENY: &[&str] = &[
        "ptrace",
        "bpf",
        "perf_event_open",
        "userfaultfd",
        "kexec_load",
        "kexec_file_load",
        "init_module",
        "finit_module",
        "delete_module",
        "keyctl",
        "add_key",
        "request_key",
        "mount",
        "umount2",
        "pivot_root",
        "swapon",
        "swapoff",
        "reboot",
        "io_uring_setup",
        "io_uring_enter",
        "io_uring_register",
        "process_vm_readv",
        "process_vm_writev",
    ];

    pub fn build() -> Result<Vec<u8>, String> {
        let mut ctx =
            ScmpFilterContext::new(ScmpAction::Allow).map_err(|e| format!("ctx: {e}"))?;
        for name in DENY {
            let sys = ScmpSyscall::from_name(name)
                .map_err(|e| format!("resolve {name}: {e}"))?;
            ctx.add_rule(ScmpAction::Errno(libc::EPERM), sys)
                .map_err(|e| format!("add_rule {name}: {e}"))?;
        }
        let mut buf: Vec<u8> = Vec::new();
        // export_bpf writes to an fd; use a memfd / pipe approach.
        // Simpler: use a tempfile then read it back.
        let mut f = tempfile::tempfile().map_err(|e| format!("tempfile: {e}"))?;
        ctx.export_bpf(&mut f).map_err(|e| format!("export_bpf: {e}"))?;
        use std::io::{Read, Seek, SeekFrom};
        f.seek(SeekFrom::Start(0)).map_err(|e| format!("seek: {e}"))?;
        f.read_to_end(&mut buf).map_err(|e| format!("read: {e}"))?;
        Ok(buf)
    }
}
