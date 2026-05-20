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
        let mtime = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
        if now.duration_since(mtime).unwrap_or(Duration::ZERO) > max_age {
            let _ = std::fs::remove_dir_all(&path);
        }
    }
}

fn set_mtime(path: &std::path::Path, when: SystemTime) {
    use std::os::unix::fs::MetadataExt;
    let secs = when
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs() as libc::time_t)
        .unwrap_or(0);
    let cpath = std::ffi::CString::new(path.to_str().unwrap()).unwrap();
    // Two-element array: [atime, mtime].
    let times = [
        libc::timeval {
            tv_sec: secs,
            tv_usec: 0,
        },
        libc::timeval {
            tv_sec: secs,
            tv_usec: 0,
        },
    ];
    let rc = unsafe { libc::utimes(cpath.as_ptr(), times.as_ptr()) };
    assert!(
        rc == 0,
        "utimes({}) failed: {}",
        path.display(),
        std::io::Error::last_os_error()
    );
    // Suppress unused-import lint.
    let _ = std::fs::metadata(path).map(|m| m.size());
}
