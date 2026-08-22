//! Acceptance tests for the "honest sandbox diagnostics" invariants, filed in
//! the SERVER crate (the lifecycle superproject) and reaching the engine
//! behavior through the same `code_sandbox::{config, sandbox, tools::execute}`
//! re-exports the production code uses. Each test carries its lifecycle TEST-ID
//! + invariant tag.

use crate::modules::code_sandbox::config;

// TEST-1 [acceptance] [invariant: INV-1] — every SandboxAvailability variant
// explains itself with a specific, honest reason, and NO variant carries the
// false "not yet booted" clause (init() is a set-once OnceCell).
#[test]
fn test1_explain_gives_specific_honest_reason_per_variant() {
    use config::SandboxAvailability::*;
    let all = [
        Ready,
        DisabledInConfig,
        HostUnsupported,
        CloudImdsRefused,
        WorkspaceInitFailed,
        PoolMissing,
        NotInitialized,
    ];
    for v in all {
        let reason = v.explain();
        assert!(!reason.is_empty(), "{v:?} must explain itself");
        assert!(
            !reason.contains("not yet booted"),
            "{v:?}: the false 'not yet booted' clause must be gone: {reason}"
        );
    }
    let host = config::SandboxAvailability::HostUnsupported.explain();
    assert!(
        host.contains("bwrap") && (host.contains("PATH") || host.contains("host")),
        "HostUnsupported must name the bwrap/host gate, not guess: {host}"
    );
    assert!(
        !host.contains("disabled"),
        "HostUnsupported must not guess 'disabled': {host}"
    );
}

// TEST-2 [acceptance] [invariant: INV-1] — the SANDBOX_NOT_INITIALIZED error
// carries the ACTUAL recorded reason and never the guessed disjunction.
#[test]
fn test2_not_initialized_error_states_the_real_reason() {
    let err = config::not_initialized_error();
    assert_eq!(err.error_code(), "SANDBOX_NOT_INITIALIZED");
    let msg = err.to_string();
    assert!(
        msg.contains(config::init_status().explain()),
        "message must include the recorded reason; got: {msg}"
    );
    assert!(
        !msg.contains("not yet booted") && !msg.contains("module disabled"),
        "message must not assert a guessed cause; got: {msg}"
    );
}

// TEST-3 [acceptance] [invariant: INV-2] — the seccomp-write classifier quiets a
// routine child-exited-early EPIPE while keeping a genuine truncation loud, and
// never mislabels a genuine truncation as ChildGone. Linux-only (the classifier
// is `#[cfg(target_os = "linux")]`).
#[cfg(target_os = "linux")]
#[test]
fn test3_classify_seccomp_write_splits_epipe_from_genuine_truncation() {
    use crate::modules::code_sandbox::sandbox::{SeccompWriteOutcome, classify_seccomp_write};
    // EPIPE == errno 32 on Linux; use the numeric value to avoid a libc dep here.
    const EPIPE: i32 = 32;
    const EIO: i32 = 5;

    assert_eq!(
        classify_seccomp_write(560, 560, None),
        SeccompWriteOutcome::Complete
    );
    assert_eq!(
        classify_seccomp_write(0, 560, Some(EPIPE)),
        SeccompWriteOutcome::ChildGone,
        "the ~204/day written=0/EPIPE case is the routine child-gone one"
    );
    assert_eq!(
        classify_seccomp_write(120, 560, Some(EPIPE)),
        SeccompWriteOutcome::ChildGone
    );
    assert_eq!(
        classify_seccomp_write(300, 560, Some(EIO)),
        SeccompWriteOutcome::Truncated
    );
    assert_eq!(
        classify_seccomp_write(0, 560, None),
        SeccompWriteOutcome::Truncated,
        "an unexpected EOF defaults to the safe loud Truncated"
    );
    assert_ne!(
        classify_seccomp_write(300, 560, Some(EIO)),
        SeccompWriteOutcome::ChildGone,
        "a genuine (non-EPIPE) truncation must NOT be quieted as ChildGone"
    );
}

// TEST-4 [acceptance] [invariant: INV-3] — redact_host_paths scrubs the
// workspace root, rootfs mount dir, AND every extra bind source
// (workflow/provider mounts + caller ro-binds), and is a no-op otherwise.
#[test]
fn test4_redact_host_paths_scrubs_all_bind_sources_and_is_noop_otherwise() {
    use crate::modules::code_sandbox::tools::execute::redact_host_paths;
    use std::path::PathBuf;

    let ws = PathBuf::from("/var/lib/ziee/sandboxes");
    let rootfs = PathBuf::from("/var/cache/ziee/sandbox-rootfs/v3-minimal/mount");
    let extra = ["/home/alice/Documents", "/data/lit-cache/view/conv-abc"];

    let text = "opened /var/lib/ziee/sandboxes/abc/data.txt from \
                /var/cache/ziee/sandbox-rootfs/v3-minimal/mount/usr/bin/cat, \
                /home/alice/Documents/report.pdf and \
                /data/lit-cache/view/conv-abc/paper.xml";
    let out = redact_host_paths(text, &ws, &rootfs, &extra);
    assert!(
        !out.contains("/var/lib/ziee/sandboxes"),
        "workspace scrubbed: {out}"
    );
    assert!(
        !out.contains("/var/cache/ziee/sandbox-rootfs"),
        "rootfs scrubbed: {out}"
    );
    assert!(
        !out.contains("/home/alice/Documents"),
        "provider mount scrubbed: {out}"
    );
    assert!(
        !out.contains("/data/lit-cache"),
        "caller ro-bind scrubbed: {out}"
    );
    assert!(out.contains("<sandbox-workspace>") && out.contains("<sandbox-rootfs>"));
    assert!(
        out.contains("<sandbox-mount>"),
        "extra-mount placeholder present: {out}"
    );

    let clean = "hello from /home/sandboxuser working in /usr/bin";
    assert_eq!(redact_host_paths(clean, &ws, &rootfs, &[]), clean);
}

// TEST-5 [acceptance] [invariant: INV-3] — a simulated bwrap dead-mount stderr
// line (rootfs mount AND a provider mount source) no longer contains the host
// absolute path after redaction, proving the isError:false success-path leak is
// closed.
#[test]
fn test5_redact_host_paths_scrubs_bwrap_dead_mount_stderr() {
    use crate::modules::code_sandbox::tools::execute::redact_host_paths;
    use std::path::PathBuf;

    let ws = PathBuf::from("/data/ziee/sandboxes");
    let rootfs = PathBuf::from("/data/ziee/rootfs-cache/current/mount");
    let extra = ["/home/bob/secret-project"];

    let rootfs_stderr = "bwrap: Can't mount /data/ziee/rootfs-cache/current/mount/usr \
                         on /newroot/usr: Transport endpoint is not connected";
    let out = redact_host_paths(rootfs_stderr, &ws, &rootfs, &extra);
    assert!(
        !out.contains("/data/ziee/rootfs-cache"),
        "the host rootfs path must not reach the model: {out}"
    );
    assert!(
        out.contains("<sandbox-rootfs>/usr"),
        "placeholder in place: {out}"
    );

    let mount_stderr = "bwrap: Can't find source path \
                        /home/bob/secret-project: No such file or directory";
    let out2 = redact_host_paths(mount_stderr, &ws, &rootfs, &extra);
    assert!(
        !out2.contains("/home/bob/secret-project"),
        "a dead provider/extra mount source must not leak to the model: {out2}"
    );
    assert!(
        out2.contains("<sandbox-mount>"),
        "placeholder in place: {out2}"
    );
}
