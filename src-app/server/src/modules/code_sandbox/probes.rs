//! One-time boot probes for the sandbox.
//!
//! Probes run exactly once at `code_sandbox::init()` and the results
//! land in `HardeningCapabilities`. Every per-call code path reads
//! that cached struct — no shellouts on the hot path.

use std::path::{Path, PathBuf};
use std::process::Command as StdCommand;

use crate::core::config::CodeSandboxConfig;
use crate::modules::code_sandbox::types::{
    CgroupMode, HardeningCapabilities, HostCapabilities, PidNsMode, SeccompMode,
};

/// Probe everything that does NOT require the sandbox rootfs to be
/// mounted: bwrap on PATH, delegated cgroup, seccomp filter compile.
/// Cost: <10 ms. Returns `None` if bwrap is missing (in which case
/// `init()` skips MCP registration entirely).
///
/// Boot path. The rootfs-dependent half (`probe_pid_ns`) runs lazily
/// on the first `execute_command` call via
/// [`runtime_mount::ensure_rootfs_ready`].
pub fn probe_host_only(config: &CodeSandboxConfig) -> Option<HostCapabilities> {
    let bwrap_path = which_bwrap()?;
    let cgroup = probe_cgroup(&config.cgroup_parent);
    let seccomp = compile_seccomp_filter();
    Some(HostCapabilities { bwrap_path, cgroup, seccomp })
}

/// Promote `HostCapabilities` to the full `HardeningCapabilities` by
/// running the rootfs-dependent probes against the now-mounted rootfs.
/// Called from `runtime_mount::ensure_rootfs_ready` after squashfuse
/// has put the rootfs at `<rootfs_path>/usr`.
///
/// Returns `Err` if `probe_pid_ns` produces `Disabled` — that's the
/// "the sandbox cannot run on this host" signal that surfaces to the
/// caller as a structured MCP error.
pub fn probe_rootfs_dependent(
    config: &CodeSandboxConfig,
    host: &HostCapabilities,
) -> Result<HardeningCapabilities, String> {
    let pid_namespace = probe_pid_ns(&host.bwrap_path, &config.rootfs_path);
    if matches!(pid_namespace, PidNsMode::Disabled) {
        return Err(format!(
            "PID-namespace probe failed against rootfs at {}; bwrap \
             cannot start a useful sandbox here. Check kernel config \
             (CONFIG_USER_NS=y, unprivileged_userns_clone=1).",
            config.rootfs_path
        ));
    }
    let caps = HardeningCapabilities {
        bwrap_path: host.bwrap_path.clone(),
        pid_namespace,
        cgroup: host.cgroup.clone(),
        seccomp: host.seccomp.clone(),
    };
    log_hardening_summary(&caps);
    Ok(caps)
}

/// Convenience: probe both host-only and rootfs-dependent halves in one
/// call. Used by Tier-4 tests and the bootstrap script, NOT by the
/// production boot path (which only calls `probe_host_only`).
pub fn probe_all(config: &CodeSandboxConfig) -> HardeningCapabilities {
    let host = match probe_host_only(config) {
        Some(h) => h,
        None => {
            tracing::error!(
                "code_sandbox: bwrap not found on PATH; sandbox will refuse \
                 to register. Install bubblewrap (apt install bubblewrap)."
            );
            return HardeningCapabilities {
                bwrap_path: PathBuf::from("bwrap"),
                pid_namespace: PidNsMode::Disabled,
                cgroup: CgroupMode::None,
                seccomp: SeccompMode::NotLinked,
            };
        }
    };
    probe_rootfs_dependent(config, &host).unwrap_or_else(|_| HardeningCapabilities {
        bwrap_path: host.bwrap_path,
        pid_namespace: PidNsMode::Disabled,
        cgroup: host.cgroup,
        seccomp: host.seccomp,
    })
}

fn log_hardening_summary(caps: &HardeningCapabilities) {
    let summary = format!(
        "code_sandbox: hardening = {{ rlimits: on, bwrap: on, pid_ns: {pid_ns}, cgroup_v2: {cg}, seccomp: {sc} }}",
        pid_ns = match caps.pid_namespace {
            PidNsMode::Strict => "on",
            PidNsMode::DevBindFallback => "off-fallback-dev-bind",
            PidNsMode::Disabled => "DISABLED",
        },
        cg = match &caps.cgroup {
            CgroupMode::Delegated(_) => "on (delegated)",
            CgroupMode::None => "off-needs-delegation",
        },
        sc = match &caps.seccomp {
            SeccompMode::Loaded(_) => "on",
            SeccompMode::NotLinked => "off-feature-not-linked",
            SeccompMode::Disabled => "off-libseccomp-failed",
        },
    );
    tracing::info!("{summary}");
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
/// disabled. Public-in-crate so `runtime_mount` can call it from the
/// lazy-init path against the just-mounted rootfs.
pub(crate) fn probe_pid_ns(bwrap_path: &Path, rootfs: &str) -> PidNsMode {
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
///
/// SECURITY: the parent path comes from `code_sandbox.cgroup_parent`
/// config. We canonicalize once at boot and require:
///   1. the canonical path is under `/sys/fs/cgroup/` — refuses to
///      operate on arbitrary filesystem paths even if the operator
///      misconfigures a symlink,
///   2. the path itself is NOT a symlink (a config-time symlink swap
///      could otherwise point us at an unrelated cgroup),
///   3. subtree_control is tokenized on whitespace (substring match
///      would accept the kernel's `-memory` denied-controller form).
fn probe_cgroup(parent_str: &str) -> CgroupMode {
    if parent_str.trim().is_empty() {
        return CgroupMode::None;
    }
    let raw_parent = PathBuf::from(parent_str);
    // Refuse symlinks at the parent path itself.
    if let Ok(meta) = std::fs::symlink_metadata(&raw_parent) {
        if meta.file_type().is_symlink() {
            tracing::warn!(
                "code_sandbox: cgroup_parent {} is a symlink; refusing for safety",
                raw_parent.display()
            );
            return CgroupMode::None;
        }
    }
    // Canonicalize and re-check the resolved path is under /sys/fs/cgroup.
    let parent = match std::fs::canonicalize(&raw_parent) {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!(
                "code_sandbox: cgroup_parent {} not accessible ({e}); rlimits-only mode",
                raw_parent.display()
            );
            return CgroupMode::None;
        }
    };
    if !parent.starts_with("/sys/fs/cgroup") {
        tracing::warn!(
            "code_sandbox: cgroup_parent resolved to {} which is NOT under \
             /sys/fs/cgroup; refusing",
            parent.display()
        );
        return CgroupMode::None;
    }
    let subtree = parent.join("cgroup.subtree_control");
    if !subtree.exists() {
        tracing::warn!(
            "code_sandbox: cgroup parent {} missing or not delegated; rlimits-only mode",
            parent.display()
        );
        return CgroupMode::None;
    }
    // Read subtree_control and tokenize properly. The kernel writes a
    // space-separated list of active-controller names (no `+`/`-`
    // prefix in the READ form, despite the WRITE syntax using prefixes).
    // A naive `contains("memory")` would match a hypothetical
    // `-memory` token (denied) or a substring like `memory_pressure`.
    let controllers = std::fs::read_to_string(&subtree).unwrap_or_default();
    let active: std::collections::HashSet<&str> = controllers.split_whitespace().collect();
    if !active.contains("memory") || !active.contains("pids") {
        tracing::warn!(
            "code_sandbox: cgroup parent {} subtree_control lacks memory+pids \
             (active={active:?}); rlimits-only mode",
            parent.display()
        );
        return CgroupMode::None;
    }
    // Sanity: can we mkdir + rmdir a probe child?
    let probe = parent.join(".sandbox-probe");
    let _ = std::fs::remove_dir(&probe); // ignore prior leftovers
    if std::fs::create_dir(&probe).is_ok() {
        let _ = std::fs::remove_dir(&probe);
        // Boot-time leak sweep: any pre-existing `sandbox-*` dirs
        // older than 1 hour are stale from previous server runs that
        // didn't get to clean up (crashes, SIGKILLs). Sweep them so
        // they don't accumulate across restarts.
        sweep_stale_cgroup_scopes(&parent);
        CgroupMode::Delegated(parent)
    } else {
        tracing::warn!(
            "code_sandbox: cgroup parent {} not writable by server uid; rlimits-only mode",
            parent.display()
        );
        CgroupMode::None
    }
}

/// Boot-time sweep of orphaned per-call cgroup scopes from prior
/// server runs. We delete `sandbox-*` subdirs older than 1 hour;
/// fresher ones are left alone to avoid racing a just-started in-flight
/// call from a hot-restart scenario.
fn sweep_stale_cgroup_scopes(parent: &std::path::Path) {
    use std::time::{Duration, SystemTime};
    const MIN_AGE: Duration = Duration::from_secs(3600);
    let Ok(entries) = std::fs::read_dir(parent) else { return };
    let mut swept = 0usize;
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else { continue };
        if !name.starts_with("sandbox-") { continue }
        let Ok(meta) = entry.metadata() else { continue };
        if !meta.is_dir() { continue }
        let age = meta.modified().ok()
            .and_then(|m| SystemTime::now().duration_since(m).ok())
            .unwrap_or(Duration::ZERO);
        if age < MIN_AGE { continue }
        if std::fs::remove_dir(&path).is_ok() {
            swept += 1;
        }
    }
    if swept > 0 {
        tracing::info!(swept, "code_sandbox: swept stale cgroup scopes at boot");
    }
}

/// Compile the seccomp filter once at boot. Active only when BOTH
/// the `code_sandbox_seccomp` cargo feature is enabled AND the build
/// target is Linux. The feature is on by default; on Mac/Windows
/// the target gate trips so the filter is a no-op and seccomp logs
/// as `off-feature-not-linked` (the same surface as if the feature
/// had been turned off explicitly).
fn compile_seccomp_filter() -> SeccompMode {
    #[cfg(not(all(feature = "code_sandbox_seccomp", target_os = "linux")))]
    {
        SeccompMode::NotLinked
    }
    #[cfg(all(feature = "code_sandbox_seccomp", target_os = "linux"))]
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

#[cfg(all(feature = "code_sandbox_seccomp", target_os = "linux"))]
mod seccomp_impl {
    use libseccomp::{ScmpAction, ScmpFilterContext, ScmpSyscall};

    /// Syscalls we deny (return EPERM).
    ///
    /// The filter is a DENYLIST with a default-ALLOW action. This is
    /// the same shape Flatpak uses. The list
    /// MUST include every syscall family that could be combined with
    /// our other isolation primitives to weaken them:
    ///   - kernel modification (kexec, *_module, swap*, reboot)
    ///   - tracing (ptrace, perf_event_open, process_vm_*)
    ///   - new-namespace / mount-API escapes (mount/umount/umount2,
    ///     setns, unshare, pivot_root, chroot, the full new-mount-API
    ///     family — fsopen/fsconfig/fsmount/move_mount/open_tree/
    ///     mount_setattr)
    ///   - newer clone variants that bypass filters checking only
    ///     `clone` (clone3 is the classic CVE-2022-23222 vector)
    ///   - kernel keyring (keyctl, add_key, request_key)
    ///   - direct I/O port access (iopl, ioperm)
    ///   - quotactl, personality, userfaultfd, bpf, io_uring*
    ///
    /// Adding to this list is always safe (more restrictive). Removing
    /// requires re-evaluating the threat model. If a workload legitimately
    /// needs one of these (e.g. R packages calling perf), prefer an
    /// allowlist filter at that point rather than weakening this one.
    /// Denied with **EPERM** — the caller has no fallback path; a hard
    /// "operation not permitted" is correct.
    const DENY_EPERM: &[&str] = &[
        // Tracing / cross-process introspection
        "ptrace",
        "perf_event_open",
        "process_vm_readv",
        "process_vm_writev",
        "pidfd_send_signal",
        "pidfd_getfd",
        "pidfd_open",
        "kcmp",
        // Kernel modification
        "bpf",
        "userfaultfd",
        "kexec_load",
        "kexec_file_load",
        "init_module",
        "finit_module",
        "delete_module",
        // Legacy module API (pre-2.6 but still present on some kernels)
        "create_module",
        "query_module",
        "get_kernel_syms",
        // Keyring
        "keyctl",
        "add_key",
        "request_key",
        // Mount family — old API
        "mount",
        "umount",
        "umount2",
        "pivot_root",
        "chroot",
        // Namespace manipulation. We already invoke bwrap with
        // --unshare-* ourselves; the sandboxed code must not be able
        // to join other namespaces or create new ones to escape.
        "setns",
        "unshare",
        // File-handle resolution — the "Shocker" container-breakout
        // primitive: resolve a host inode by handle, bypassing the
        // mount-namespace path view. (CAP_DAC_READ_SEARCH-gated, so the
        // userns mostly blocks it already; deny anyway, defense-in-depth.)
        "open_by_handle_at",
        "name_to_handle_at",
        // Time is NOT namespaced — block wall-clock tampering.
        "clock_settime",
        "clock_adjtime",
        "settimeofday",
        "stime",
        // Kernel ring buffer — leaks KASLR pointers.
        "syslog",
        // Swap / power
        "swapon",
        "swapoff",
        "reboot",
        // I/O ring — recent escape vectors via io_uring SQE rewrites
        "io_uring_setup",
        "io_uring_enter",
        "io_uring_register",
        // Direct hardware I/O ports
        "iopl",
        "ioperm",
        // Quotas + execution-domain switching + process accounting
        "quotactl",
        "personality",
        "acct",
        // NUMA memory-policy (best-effort in libs → EPERM tolerated)
        "migrate_pages",
        "mbind",
        "get_mempolicy",
        "set_mempolicy",
        "move_pages",
        // Misc obsolete / dangerous
        "lookup_dcookie",
        "nfsservctl",
        "uselib",
        "ustat",
        "vm86",
        "vm86old",
        "modify_ldt",
    ];

    /// Denied with **ENOSYS** — callers (glibc, runtimes) *probe* these
    /// and fall back to the legacy syscall only on `ENOSYS`. Returning
    /// `EPERM` here would defeat the fallback and break threaded/forked
    /// workloads (e.g. R `parallel::mclapply`, `data.table`, `torch` on
    /// glibc ≥ 2.34). Matches Flatpak's handling.
    const DENY_ENOSYS: &[&str] = &[
        // Newer clone — the namespace-escape bypass when a filter only
        // blocks `clone` (CVE-2022-23222 family). glibc probes clone3
        // then falls back to clone on ENOSYS.
        "clone3",
        // Mount family — new API (Linux 5.2+); a denylist missing these
        // would let a sandboxee construct mounts via the new API and
        // bypass the read-only rootfs binds. Probe-and-fallback callers
        // expect ENOSYS.
        "fsopen",
        "fsconfig",
        "fsmount",
        "move_mount",
        "open_tree",
        "mount_setattr",
    ];

    pub fn build() -> Result<Vec<u8>, String> {
        let mut ctx =
            ScmpFilterContext::new_filter(ScmpAction::Allow).map_err(|e| format!("ctx: {e}"))?;
        // Best-effort per-syscall: if a name doesn't resolve on this
        // host kernel (newer syscalls on old kernels, or vice versa),
        // log it and continue with the rest. A failed lookup must NOT
        // disable the WHOLE filter — that would be a catastrophic
        // hardening regression for the sake of one missing entry.
        let mut resolved = 0;
        let mut unresolved: Vec<&str> = Vec::new();
        for (action, names) in [
            (ScmpAction::Errno(libc::EPERM), DENY_EPERM),
            (ScmpAction::Errno(libc::ENOSYS), DENY_ENOSYS),
        ] {
            for name in names {
                let sys = match ScmpSyscall::from_name(name) {
                    Ok(s) => s,
                    Err(_) => {
                        unresolved.push(*name);
                        continue;
                    }
                };
                ctx.add_rule(action, sys)
                    .map_err(|e| format!("add_rule {name}: {e}"))?;
                resolved += 1;
            }
        }
        if !unresolved.is_empty() {
            tracing::warn!(
                resolved,
                total = DENY_EPERM.len() + DENY_ENOSYS.len(),
                unresolved = ?unresolved,
                "code_sandbox: some seccomp DENY entries did not resolve \
                 on this kernel; they are silently skipped. Verify kernel \
                 version supports the listed syscalls (e.g. clone3 needs >=5.3)."
            );
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

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn build_produces_nonempty_bpf() {
            let bpf = build().expect("seccomp filter builds");
            assert!(!bpf.is_empty(), "exported BPF must be non-empty");
        }

        #[test]
        fn deny_lists_are_disjoint_and_classified_correctly() {
            assert!(!DENY_EPERM.is_empty());
            assert!(!DENY_ENOSYS.is_empty());
            for n in DENY_ENOSYS {
                assert!(!DENY_EPERM.contains(n), "{n} must not be in both lists");
            }
            // Probe-and-fallback syscalls MUST be ENOSYS so glibc falls back
            // (else threaded/forked R breaks under seccomp).
            for n in ["clone3", "fsopen", "fsconfig", "fsmount", "move_mount", "open_tree", "mount_setattr"] {
                assert!(DENY_ENOSYS.contains(&n), "{n} must be in DENY_ENOSYS");
            }
            // High-value breakout primitive must be denied.
            assert!(DENY_EPERM.contains(&"open_by_handle_at"));
            assert!(DENY_EPERM.contains(&"name_to_handle_at"));
        }
    }
}
