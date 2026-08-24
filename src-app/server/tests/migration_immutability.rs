//! A migration that has been SHIPPED is immutable. This guards that.
//!
//! ## The failure this prevents
//!
//! Editing a migration that some database has already applied makes that
//! database FAIL TO BOOT. `sqlx::migrate!` checksums each migration against
//! `_sqlx_migrations`, and the migrator is configured with
//! `set_ignore_missing(true)` only — which ignores migrations present in the DB
//! but absent from source and does NOT disable checksum validation. A mismatch
//! aborts migrations, so the embedded server never starts, every API call
//! fails, and with no session the UI falls back to first-run setup. It looks
//! exactly like an auth or network fault, and is neither.
//!
//! This was hit for real downstream: a one-line edit to an already-shipped seed
//! migration bricked every upgraded install. A FRESH install of the same build
//! was fine — which is why it shipped, because every test here runs against a
//! fresh database, leaving the upgrade path (the one real users take) untested.
//!
//! ## WHAT THIS TEST DOES NOT COVER — read before trusting it
//!
//! This is the cheap guard. Be clear about its limits:
//!
//! 1. **It does not execute any migration.** It compares file BYTES. It cannot
//!    catch a migration that is textually new but semantically broken, and it
//!    does not prove an upgraded database actually migrates. **Nothing in this
//!    repo currently proves that.**
//! 2. **Its baseline is what was COMMITTED**, not what was BUILT. A build cut
//!    from a local, unpushed commit is invisible to it.
//! 3. **It is skip-on-unavailable**, loudly. Without git it prints why and
//!    passes rather than failing the suite.
//!
//! A real two-stage test — apply the previously-shipped set, then the current
//! set on top, against a live database — would cover (1) and (2). It needs the
//! shared test harness's database bootstrap, so it is left as a follow-up rather
//! than built here.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::process::Command;

fn git(repo_root: &std::path::Path, args: &[&str]) -> Option<String> {
    let out = Command::new("git")
        .current_dir(repo_root)
        .args(args)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).into_owned())
}

fn repo_root() -> Option<PathBuf> {
    let here = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let out = Command::new("git")
        .current_dir(&here)
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    Some(PathBuf::from(
        String::from_utf8_lossy(&out.stdout).trim().to_string(),
    ))
}

fn is_migration(path: &str) -> bool {
    path.contains("/migrations/") && path.ends_with(".sql")
}

/// Migrations already edited-after-commit in history, before this rule existed.
///
/// All five are pre-existing on `main` and untouched by the branch that added
/// this guard. They are grandfathered rather than "fixed": rewriting their bytes
/// now would ITSELF be an edit to a shipped migration, which is the exact hazard
/// this file exists to prevent.
///
/// **This list may only ever SHRINK.** A new entry means someone edited a
/// shipped migration — add a new migration instead. If one of these is ever
/// superseded and deleted, drop its line.
const GRANDFATHERED: &[&str] = &[
    "src-app/server/src/modules/chat/migrations/202607200400_message_completion_state.sql",
    "src-app/server/src/modules/file/migrations/202607144125_file_fkeys.sql",
    "src-app/server/src/modules/memory/migrations/202607145050_memory_seed.sql",
    "src-app/server/src/modules/notification/migrations/202607144180_notification_fkeys.sql",
    "src-app/server/src/modules/agent/migrations/202608210100_agent_task_list_reconcile.sql",
];

#[test]
fn a_shipped_migration_is_never_edited() {
    let Some(root) = repo_root() else {
        eprintln!(
            "SKIP: not a git checkout (or git unavailable) — the shipped-migration \
             immutability guard cannot establish a baseline here."
        );
        return;
    };

    // The rule is exact: a migration's bytes must never change from what was
    // FIRST COMMITTED for that file. Comparing against a branch head instead
    // would be wrong in both directions — it would miss an edit that was itself
    // already pushed, and it would flag a legitimate RESTORE of a bad edit back
    // to the originally shipped bytes (which is precisely the FB-11 repair).
    //
    // First-appearance is also baseline-independent: it works on main, on a
    // feature branch, and in a fresh clone, with no ref to choose.
    let Some(tracked) = git(&root, &["ls-files", "--", "src-app", "sdk"]) else {
        eprintln!("SKIP: could not list tracked files.");
        return;
    };

    let mut changed: BTreeSet<String> = BTreeSet::new();
    let mut compared = 0usize;

    for path in tracked.lines().filter(|p| is_migration(p)) {
        // The commit that ADDED this file. `--follow` is deliberately not used:
        // a rename is a different question from an edit, and following one would
        // make the comparison depend on rename detection heuristics.
        let Some(history) = git(
            &root,
            &["log", "--diff-filter=A", "--format=%H", "--", path],
        ) else {
            continue;
        };
        let Some(first_commit) = history.split_whitespace().last() else {
            // Not committed yet (a migration added in the working tree) — there
            // is no shipped state to be immutable against.
            continue;
        };

        let Some(original) = git(&root, &["show", &format!("{first_commit}:{path}")]) else {
            continue;
        };
        let Ok(current) = std::fs::read_to_string(root.join(path)) else {
            continue;
        };
        compared += 1;
        if original != current && !GRANDFATHERED.contains(&path) {
            changed.insert(format!("{path}  (differs from its first commit {})", &first_commit[..9]));
        }
    }

    assert!(
        compared > 50,
        "the guard compared only {compared} migration files — the path predicate or \
         the history lookup has drifted and this check is silently passing on nothing",
    );

    assert!(
        changed.is_empty(),
        "A SHIPPED migration was edited. Any install that already applied it will \
         FAIL TO BOOT on a checksum mismatch — migrations abort, the embedded server \
         never starts, and the UI shows \"Load failed\" plus first-run setup.\n\n\
         Add a NEW migration with a later timestamp in the same module sequence \
         instead; treat every committed migration as immutable.\n\n\
         Edited:\n  {}",
        changed.into_iter().collect::<Vec<_>>().join("\n  "),
    );
}

/// The grandfather list must stay honest: every entry must still name a tracked
/// migration that genuinely differs from its first commit. A stale entry would
/// quietly widen the exemption.
#[test]
fn the_grandfather_list_contains_no_stale_entries() {
    let Some(root) = repo_root() else {
        eprintln!("SKIP: not a git checkout.");
        return;
    };
    for path in GRANDFATHERED {
        let on_disk = root.join(path);
        assert!(
            on_disk.is_file(),
            "grandfathered migration {path} no longer exists — remove it from the list",
        );
        let Some(history) = git(&root, &["log", "--diff-filter=A", "--format=%H", "--", path])
        else {
            eprintln!("SKIP: no history available for {path}");
            return;
        };
        let Some(first_commit) = history.split_whitespace().last() else {
            continue;
        };
        let original = git(&root, &["show", &format!("{first_commit}:{path}")])
            .expect("first-commit blob");
        let current = std::fs::read_to_string(&on_disk).expect("read migration");
        assert_ne!(
            original, current,
            "grandfathered migration {path} now MATCHES its first commit — it no longer \
             needs an exemption, so remove it from GRANDFATHERED (the list may only shrink)",
        );
    }
}

/// The guard is only meaningful if it can actually see migration files. A
/// baseline listing that matched nothing would make the test above pass
/// vacuously forever.
#[test]
fn the_guard_can_see_migration_files_at_all() {
    let Some(root) = repo_root() else {
        eprintln!("SKIP: not a git checkout.");
        return;
    };
    let Some(base) = ["origin/main"]
        .iter()
        .find(|r| git(&root, &["rev-parse", "--verify", r]).is_some())
    else {
        eprintln!("SKIP: no origin/main to enumerate.");
        return;
    };
    let listing = git(&root, &["ls-tree", "-r", "--name-only", base])
        .expect("ls-tree on an existing ref");
    let count = listing.lines().filter(|p| is_migration(p)).count();
    assert!(
        count > 50,
        "expected the baseline to contain many migration files, found {count} — \
         the path predicate has probably drifted and the immutability guard is \
         silently checking nothing",
    );
}
