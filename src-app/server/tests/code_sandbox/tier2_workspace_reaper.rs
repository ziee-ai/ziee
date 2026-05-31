//! Tier 2 — workspace reaper behavior.
//!
//! The reaper is a tokio task spawned from `code_sandbox::init()`; we
//! test its core predicate (mtime > 30 days → remove) directly via
//! the same `std::fs` calls the reaper uses. Async task lifecycle is
//! covered by the init smoke tests in Tier 3.

use std::time::{Duration, SystemTime};

#[test]
fn reaper_predicate_removes_old_subdirs_keeps_fresh_ones() {
    let root = tempfile::tempdir().expect("tempdir");
    let now = SystemTime::now();

    let fresh = root.path().join("fresh-conv");
    let stale = root.path().join("stale-conv");
    std::fs::create_dir(&fresh).unwrap();
    std::fs::create_dir(&stale).unwrap();

    // Set mtimes: fresh = 1 day old, stale = 45 days old.
    set_mtime(&fresh, now - Duration::from_secs(24 * 60 * 60));
    set_mtime(&stale, now - Duration::from_secs(45 * 24 * 60 * 60));

    // Replay the reaper's predicate inline.
    reap_once(root.path(), Duration::from_secs(30 * 24 * 60 * 60));

    assert!(
        fresh.exists(),
        "fresh-conv (1 day old) must be kept"
    );
    assert!(
        !stale.exists(),
        "stale-conv (45 days old) must be removed"
    );
}

#[test]
fn reaper_ignores_regular_files_in_root() {
    let root = tempfile::tempdir().expect("tempdir");
    let stray = root.path().join("not-a-conv-dir.txt");
    std::fs::write(&stray, "x").unwrap();
    set_mtime(&stray, SystemTime::now() - Duration::from_secs(60 * 24 * 60 * 60));

    reap_once(root.path(), Duration::from_secs(30 * 24 * 60 * 60));

    assert!(stray.exists(), "regular files at the root must be untouched");
}

// ─── New regression tests covering the post-audit reaper logic ─────

/// The reaper MUST skip the shared-subsystem dirs `attachments/` and
/// `identity/` even if their mtimes are very old. Without this guard
/// (added in commit a3fc827), the boot-time identity dir + the
/// shared attachment-stage dir would be wiped after 30 days of
/// server uptime, breaking subsequent sandbox calls until re-staged.
#[test]
fn reaper_preserves_attachments_and_identity_subdirs() {
    let root = tempfile::tempdir().expect("tempdir");
    let now = SystemTime::now();
    let ancient = now - Duration::from_secs(365 * 24 * 60 * 60);

    let attachments = root.path().join("attachments");
    let identity = root.path().join("identity");
    let stale_conv = root.path().join("stale-conv");
    std::fs::create_dir(&attachments).unwrap();
    std::fs::create_dir(&identity).unwrap();
    std::fs::create_dir(&stale_conv).unwrap();
    set_mtime(&attachments, ancient);
    set_mtime(&identity, ancient);
    set_mtime(&stale_conv, ancient);

    reap_once(root.path(), Duration::from_secs(30 * 24 * 60 * 60));

    assert!(attachments.exists(), "attachments/ MUST be preserved (shared)");
    assert!(identity.exists(), "identity/ MUST be preserved (shared)");
    assert!(!stale_conv.exists(), "stale conversation dir should still be reaped");
}

/// The reaper prefers the `.last_used` sentinel timestamp over the
/// directory mtime. Without this, a long-running conversation that
/// only reads/edits existing files (so the dir mtime stays stale)
/// would be reaped mid-flight after 30 days of activity. The fix in
/// commit a3fc827 has every `run_in_sandbox` call touch `.last_used`.
#[test]
fn reaper_honors_last_used_sentinel_over_dir_mtime() {
    let root = tempfile::tempdir().expect("tempdir");
    let now = SystemTime::now();
    let ancient = now - Duration::from_secs(365 * 24 * 60 * 60);
    let fresh = now - Duration::from_secs(60); // 1 minute ago

    // Conversation directory has an ANCIENT mtime (the dir was
    // created 1 year ago; no one has added/removed files since), but
    // its `.last_used` sentinel was touched 1 minute ago by a recent
    // sandbox call.
    let conv = root.path().join("active-conv");
    std::fs::create_dir(&conv).unwrap();
    let sentinel = conv.join(".last_used");
    std::fs::write(&sentinel, now.duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs().to_string()).unwrap();
    set_mtime(&sentinel, fresh);
    set_mtime(&conv, ancient);

    reap_once(root.path(), Duration::from_secs(30 * 24 * 60 * 60));

    assert!(
        conv.exists(),
        "active conversation (fresh .last_used) MUST NOT be reaped"
    );
}

// ─── Helpers — mirrors the reaper body in mod.rs::workspace_reaper ──

fn reap_once(root: &std::path::Path, max_age: Duration) {
    let now = SystemTime::now();
    let Ok(entries) = std::fs::read_dir(root) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(meta) = entry.metadata() else { continue };
        if !meta.is_dir() {
            continue;
        }
        // Skip shared subsystem dirs.
        if let Some(name) = path.file_name().and_then(|n| n.to_str())
            && (name == "attachments" || name == "identity") {
                continue;
            }
        // Prefer the `.last_used` sentinel over the directory mtime.
        let sentinel = path.join(".last_used");
        let mtime = std::fs::metadata(&sentinel)
            .and_then(|m| m.modified())
            .or_else(|_| meta.modified())
            .unwrap_or(SystemTime::UNIX_EPOCH);
        if now.duration_since(mtime).unwrap_or(Duration::ZERO) > max_age {
            let _ = std::fs::remove_dir_all(&path);
        }
    }
}

fn set_mtime(path: &std::path::Path, when: SystemTime) {
    // Cross-platform mtime setter, working for BOTH files and
    // directories. The earlier `OpenOptions::write(true).read(true)
    // .open(path)` approach was POSIX-non-compliant for directories
    // — `open(O_RDWR, dir)` returns `EISDIR` on every Unix per
    // `open(2)`. The Windows side worked because Windows lets you
    // open a directory handle when `FILE_FLAG_BACKUP_SEMANTICS` is
    // set; the Unix branch never worked for directories on any host.
    //
    // Replaced with `utimensat` (Unix) and a fd-based `set_modified`
    // with FILE_FLAG_BACKUP_SEMANTICS (Windows). `utimensat` operates
    // on the inode by path — no fd needed, no read/write access
    // required — and is supported on Linux 2.6.22+, every macOS,
    // every BSD. Both atime and mtime are set to `when` because the
    // reaper compares against mtime only and there's no callsite that
    // observes atime, so a single value keeps the helper simple.
    let dur = when
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    #[cfg(windows)]
    {
        // Windows path uses the file handle approach because Win32
        // SetFileTime() needs HANDLE; we can't go path-based without
        // CreateFileW. Existing logic is correct on Windows — kept.
        let _ = dur;
        use std::os::windows::fs::OpenOptionsExt;
        // 0x02000000 = FILE_FLAG_BACKUP_SEMANTICS (required for directory handles)
        // 0x40000000 = GENERIC_WRITE
        // 0x80000000 = GENERIC_READ
        let f = std::fs::OpenOptions::new()
            .access_mode(0x80000000 | 0x40000000)
            .custom_flags(0x02000000)
            .open(path)
            .unwrap_or_else(|e| panic!("open({}) for set_mtime: {e}", path.display()));
        f.set_modified(when)
            .unwrap_or_else(|e| panic!("set_modified({}): {e}", path.display()));
        return;
    }
    #[cfg(unix)]
    {
        use std::ffi::CString;
        use std::os::unix::ffi::OsStrExt;
        let c_path = CString::new(path.as_os_str().as_bytes())
            .unwrap_or_else(|_| panic!("path contains nul: {}", path.display()));
        let ts = libc::timespec {
            tv_sec: dur.as_secs() as libc::time_t,
            tv_nsec: dur.subsec_nanos() as i64,
        };
        // [atime, mtime] — same value for both, see comment above.
        let times = [ts, ts];
        // SAFETY: c_path and times outlive the call; flags=0 means
        // "follow symlinks" (test fixtures never use symlinks here);
        // AT_FDCWD makes the path relative to cwd or absolute as-is.
        let rc = unsafe {
            libc::utimensat(libc::AT_FDCWD, c_path.as_ptr(), times.as_ptr(), 0)
        };
        if rc != 0 {
            panic!(
                "utimensat({}) failed: {}",
                path.display(),
                std::io::Error::last_os_error()
            );
        }
    }
}
