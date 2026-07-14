// Shared per-worktree build-DB derivation. Chunk sdk-batteries (decision #11):
// moved into the SDK's `ziee-build-support` crate (a build-dependency) so ziee
// AND a second app share one implementation (was a `#[path]`-include of the
// server crate's build_helper/worktree_db.rs).
use ziee_build_support::worktree_db;

fn main() {
    // Point this crate's sqlx::query! compile-time verification at the SAME
    // per-worktree build database the server crate's build.rs provisions +
    // migrates (ziee-desktop depends on `ziee`, so the server's build.rs —
    // which applies BOTH server and desktop migrations — runs first). Without
    // this, the desktop crate's macros would validate against the now-stale
    // shared `postgres` db that the server build.rs no longer touches when
    // auto-isolating. Mirrors server/build.rs; honors an explicit override.
    println!("cargo:rerun-if-env-changed=DATABASE_URL");
    println!("cargo:rerun-if-env-changed=ZIEE_BUILD_DB_PERWORKTREE");
    let explicit = std::env::var("DATABASE_URL").ok();
    if worktree_db::should_auto_isolate(&explicit) {
        let base = explicit
            .unwrap_or_else(|| worktree_db::DEFAULT_BUILD_DB_URL.to_string());
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default();
        let db_name = format!("ziee_build_{}", worktree_db::worktree_key(&manifest_dir));
        let url = worktree_db::with_database(&base, &db_name);
        println!("cargo:rustc-env=DATABASE_URL={url}");
    }

    // Release-only sanity: rust_embed silently produces an empty
    // asset table when its target folder is missing or empty, which
    // ships a binary that serves "Not Found" on every non-API URL.
    // Catch the misbuild here.
    //
    // Skipped in debug because `npm run build` doesn't run before
    // `cargo build` in dev; `proxy_to_vite` covers the SPA surface
    // instead.
    if std::env::var("PROFILE").as_deref() == Ok("release") {
        let dist_index = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../ui/dist/index.html");
        if !dist_index.exists() {
            panic!(
                "release build expects the desktop UI bundle at {}, but it doesn't exist. \
                 Run `npm run build --workspace=@ziee/desktop-ui` before `cargo build --release`.",
                dist_index.display()
            );
        }
    }

    tauri_build::build()
}
