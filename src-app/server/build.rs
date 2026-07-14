use std::env;
use std::path::PathBuf;

#[path = "build_helper/pandoc.rs"]
mod pandoc;
#[path = "build_helper/typst.rs"]
mod typst;
#[path = "build_helper/pdfium.rs"]
mod pdfium;
#[path = "build_helper/uv.rs"]
mod uv;
#[path = "build_helper/bun.rs"]
mod bun;
#[path = "build_helper/biomcp.rs"]
mod biomcp;
#[path = "build_helper/sandbox_runtime.rs"]
mod sandbox_runtime;
#[path = "build_helper/wsl2_agent.rs"]
mod wsl2_agent;
#[path = "build_helper/pgvector.rs"]
mod pgvector_build;
#[path = "build_helper/worktree_db.rs"]
mod worktree_db;
// hub_seed runs LAST inside setup_external_binaries and PANICS on
// failure (unlike the helpers above, which warn-and-continue). See
// the divider comment in setup_external_binaries() for the rationale.
#[path = "build_helper/hub_seed.rs"]
mod hub_seed;

/// Redact the password portion of a postgres URL for safe logging.
/// Closes 14-core F-02 (Critical): build.rs previously echoed the raw
/// DATABASE_URL (including the password) to stderr on connection
/// failure. Build output is commonly captured in CI logs and developer
/// terminals, so the password leaked into observable surfaces.
fn redact_database_url(url: &str) -> String {
    // postgres://user:PASSWORD@host:port/db — replace the segment
    // between the first ':' after the scheme and the '@' with '***'.
    if let Some(scheme_end) = url.find("://") {
        let after_scheme = &url[scheme_end + 3..];
        if let Some(at_pos) = after_scheme.find('@') {
            let userinfo = &after_scheme[..at_pos];
            if let Some(colon) = userinfo.find(':') {
                let mut out = String::with_capacity(url.len());
                out.push_str(&url[..scheme_end + 3]);
                out.push_str(&userinfo[..colon]);
                out.push_str(":***");
                out.push_str(&after_scheme[at_pos..]);
                return out;
            }
        }
    }
    url.to_string()
}

/// Chunk BA-full: compose the merged migration directory used by BOTH the
/// build-DB provisioner (this file) and the runtime `sqlx::migrate!`
/// (`core/database/mod.rs`). It is `<manifest>/migrations-merged` =
/// the app's own `migrations/` ∪ `ziee-auth`'s structural auth-table
/// `migrations/` (path-dep at `../../sdk/crates/ziee-auth`). Files keep their
/// original version-numbered names, so the merged set version-sorts back into
/// ziee's exact `_sqlx_migrations` history — deployed DBs are unaffected.
///
/// `migrations-merged/` is a generated artifact (gitignored). build.rs always
/// runs before the crate compiles, so the dir is populated before the runtime
/// `sqlx::migrate!("./migrations-merged")` macro embeds it.
fn compose_merged_migrations() {
    let manifest = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let merged = manifest.join("migrations-merged");
    let _ = std::fs::remove_dir_all(&merged);
    std::fs::create_dir_all(&merged).expect("build.rs: create migrations-merged failed");

    let sources = [
        manifest.join("migrations"),
        manifest.join("../../sdk/crates/ziee-auth/migrations"),
    ];
    for src in &sources {
        println!("cargo:rerun-if-changed={}", src.display());
        let entries = std::fs::read_dir(src).unwrap_or_else(|e| {
            panic!(
                "build.rs: cannot read migration source {}: {}",
                src.display(),
                e
            )
        });
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("sql") {
                let name = path.file_name().expect("migration file name");
                let dst = merged.join(name);
                std::fs::copy(&path, &dst).unwrap_or_else(|e| {
                    panic!("build.rs: copy {} → merged failed: {}", path.display(), e)
                });
            }
        }
    }
    println!("cargo:rerun-if-changed={}", merged.display());
}

#[tokio::main]
async fn main() {
    // Get DATABASE_URL or use local dev fallback (build-time only;
    // the dev fallback's password matches docker-compose.yaml). The
    // fallback is for ergonomics — developers shouldn't have to set
    // DATABASE_URL just to run cargo build. The string never reaches
    // the produced binary; it's used only during the build to verify
    // SQLx queries compile.
    println!("cargo:rerun-if-env-changed=DATABASE_URL");
    println!("cargo:rerun-if-env-changed=ZIEE_BUILD_DB_PERWORKTREE");

    let explicit = env::var("DATABASE_URL").ok();

    // Per-worktree build-DB isolation: when DATABASE_URL is the committed
    // docker-compose default (the sentinel — see build_helper/worktree_db.rs),
    // give THIS worktree its own database on the same :54321 cluster so a
    // concurrent build in another worktree can't wipe our schema mid-build.
    // A genuine operator/CI override (any other DATABASE_URL) is honored
    // unchanged. Opt out with ZIEE_BUILD_DB_PERWORKTREE=0.
    let database_url = if worktree_db::should_auto_isolate(&explicit) {
        let base = explicit
            .clone()
            .unwrap_or_else(|| worktree_db::DEFAULT_BUILD_DB_URL.to_string());
        let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap_or_default();
        let db_name = format!("ziee_build_{}", worktree_db::worktree_key(&manifest_dir));
        // Ensure the per-worktree database exists (connect to the cluster's
        // maintenance `postgres` db, CREATE if missing — idempotent + race
        // tolerant: a concurrent creator just makes our CREATE a no-op error
        // we swallow). Then point the build at it.
        let admin_url = worktree_db::with_database(&base, "postgres");
        match sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .connect(&admin_url)
            .await
        {
            Ok(admin) => {
                let exists: Option<(i32,)> = sqlx::query_as(
                    "SELECT 1 FROM pg_database WHERE datname = $1",
                )
                .bind(&db_name)
                .fetch_optional(&admin)
                .await
                .ok()
                .flatten();
                if exists.is_none() {
                    // CREATE DATABASE can't run in a tx; ignore the
                    // duplicate_database error if another worktree's build
                    // raced us to it.
                    let _ = sqlx::query(&format!("CREATE DATABASE {db_name}"))
                        .execute(&admin)
                        .await;
                }
                admin.close().await;
                println!(
                    "build.rs: per-worktree build DB → {} (set ZIEE_BUILD_DB_PERWORKTREE=0 to disable)",
                    db_name
                );
                worktree_db::with_database(&base, &db_name)
            }
            Err(e) => {
                // Couldn't reach the cluster to provision — fall back to the
                // base URL and let the connect-below surface the real error.
                eprintln!("build.rs: per-worktree DB provisioning skipped: {e}");
                base
            }
        }
    } else {
        explicit.unwrap_or_else(|| worktree_db::DEFAULT_BUILD_DB_URL.to_string())
    };

    // Connect to the database
    let pool = match sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .connect(&database_url)
        .await
    {
        Ok(pool) => pool,
        Err(e) => {
            // SECURITY: redact the password before printing the URL to
            // stderr. Build output is commonly captured in CI logs and
            // developer terminals. Closes 14-core F-02 (Critical).
            eprintln!("\nERROR: Failed to connect to database: {}", e);
            eprintln!("DATABASE_URL: {}", redact_database_url(&database_url));
            panic!("Database connection failed");
        }
    };

    // Wipe the database
    sqlx::query("DROP SCHEMA IF EXISTS public CASCADE")
        .execute(&pool)
        .await
        .ok();

    sqlx::query("CREATE SCHEMA public")
        .execute(&pool)
        .await
        .expect("Failed to create schema");

    sqlx::query("GRANT ALL ON SCHEMA public TO PUBLIC")
        .execute(&pool)
        .await
        .ok();

    // Chunk BA-full: compose the MERGED migration directory
    // (`migrations-merged` = the app's own `migrations/` ∪ `ziee-auth`'s
    // structural auth-table migrations, both keeping their original version
    // numbers + byte content). The auth-table migrations moved into
    // `ziee-auth` (which owns them + its `query!` macros); the merged,
    // version-sorted set reproduces ziee's exact `_sqlx_migrations` history
    // so deployed DBs are unaffected. BOTH this build-DB provisioner AND the
    // runtime `sqlx::migrate!` (core/database/mod.rs) point at the merged dir.
    compose_merged_migrations();

    // Run migrations — the merged server set AND the desktop tauri crate's.
    // The desktop crate's `remote_access` + `magic_link` modules
    // use `sqlx::query_as!()` macros that need the build DB to
    // include their schema (remote_access_settings, magic_link_tokens).
    // Folding both migration dirs into a single Migrator means the
    // shared build DB on port 54321 has every table both crates
    // touch, and macro validation succeeds for either crate.
    for migrations_dir_rel in ["migrations-merged", "../desktop/tauri/migrations"] {
        let migrations_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(migrations_dir_rel);
        if !migrations_path.exists() {
            // Desktop migrations are an optional second source — log
            // and skip if the directory hasn't been created yet.
            eprintln!(
                "build.rs: skipping migrations path {} (not present)",
                migrations_path.display()
            );
            continue;
        }
        // Tell cargo to re-run build.rs whenever migration files
        // change in EITHER directory.
        println!("cargo:rerun-if-changed={}", migrations_path.display());

        let display = migrations_path.display().to_string();
        let mut migrator = sqlx::migrate::Migrator::new(migrations_path)
            .await
            .unwrap_or_else(|e| {
                panic!("Failed to create migrator for {}: {}", display, e)
            });
        // Each migrator's source dir only knows about its own migrations.
        // After the first migrator runs server's, the second migrator
        // (for desktop) sees server's version rows in _sqlx_migrations
        // and would error with "previously applied but is missing in
        // the resolved migrations". `set_ignore_missing(true)` tells
        // it to ignore versions outside its source dir — the same flag
        // run_desktop_migrations() in backend/mod.rs uses at runtime
        // for the same reason.
        migrator.set_ignore_missing(true);

        if let Err(e) = migrator.run(&pool).await {
            eprintln!("\nERROR: Migration failed for {}: {}", display, e);
            panic!("Migration failed");
        }
    }

    pool.close().await;

    // Set DATABASE_URL for SQLx compile-time verification
    println!("cargo:rustc-env=DATABASE_URL={}", database_url);

    // Generate chat repository with extension fields
    generate_chat_repository();

    // Download Pandoc and PDFium binaries
    setup_external_binaries();
}

fn setup_external_binaries() {
    println!("=== SETUP_EXTERNAL_BINARIES CALLED ===");

    let target = env::var("TARGET").unwrap();
    let out_dir = env::var("OUT_DIR").unwrap();

    println!("TARGET: {}", target);
    println!("OUT_DIR: {}", out_dir);

    // Use server/binaries/{target}/ for embedding
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let binaries_dir = PathBuf::from(&manifest_dir)
        .join("binaries")
        .join(&target);

    println!("Setting up external binaries for embedding at: {:?}", binaries_dir);

    // Setup Pandoc - downloads to binaries/{target}/
    if let Err(e) = pandoc::setup_pandoc(&target, &binaries_dir, &out_dir) {
        eprintln!("Warning: Failed to setup Pandoc: {}", e);
    }

    // Setup typst - downloads to binaries/{target}/typst/.
    // typst is the Unicode-capable PDF engine pandoc routes office-doc
    // → PDF conversions through (replaces pdflatex, which choked on
    // common Unicode characters like ≥/≤/→/π). Self-contained binary,
    // no system TeX install required.
    if let Err(e) = typst::setup_typst(&target, &binaries_dir, &out_dir) {
        eprintln!("Warning: Failed to setup typst: {}", e);
    }

    // Setup PDFium - downloads to binaries/{target}/
    if let Err(e) = pdfium::setup_pdfium(&target, &binaries_dir, &out_dir) {
        eprintln!("Warning: Failed to setup PDFium: {}", e);
    }

    // Setup UV - downloads to binaries/{target}/
    if let Err(e) = uv::setup_uv(&target, &binaries_dir, &out_dir) {
        eprintln!("Warning: Failed to setup UV: {}", e);
    }

    // Setup Bun - downloads to binaries/{target}/
    if let Err(e) = bun::setup_bun(&target, &binaries_dir, &out_dir) {
        eprintln!("Warning: Failed to setup Bun: {}", e);
    }

    // Setup BioMCP - downloads to binaries/{target}/. Fail-soft like
    // pgvector: on failure a zero-byte stub is staged so the runtime
    // `include_bytes!` compiles and the bio_mcp module self-disables.
    if let Err(e) = biomcp::setup_biomcp(&target, &binaries_dir, &out_dir) {
        eprintln!("Warning: Failed to setup BioMCP: {}", e);
    }
    // Re-run the helper when its source changes (e.g. a BIOMCP_VERSION bump),
    // so a version change re-fetches even if nothing else triggered build.rs.
    println!("cargo:rerun-if-changed=build_helper/biomcp.rs");

    // Assemble the macOS sandbox runtime bundle (no-op on every other
    // target). Failures here are warnings, not hard errors — a dev
    // machine without Docker can still build the server; the embedded
    // bundle just won't work at runtime (the existing env/exe-parent
    // fallbacks in mac_vm.rs cover the dev path).
    if let Err(e) = sandbox_runtime::setup(&target, &binaries_dir, &out_dir) {
        eprintln!("Warning: Failed to assemble sandbox-runtime bundle: {}", e);
    }

    // Cross-compile the Linux sandbox-guest-agent for Windows release
    // builds (no-op on every other target). Same fail-soft contract as
    // the mac path: a dev box without Docker still builds the server;
    // the runtime falls back to the sibling-of-exe agent path that
    // `scripts/build-sandbox-agent-linux.sh` produces.
    if let Err(e) = wsl2_agent::setup(&target, &binaries_dir, &out_dir) {
        eprintln!("Warning: Failed to bundle WSL2 sandbox-guest-agent: {}", e);
    }

    // Setup pgvector — fail-soft. If the build fails (missing make,
    // missing pgvector submodule, network failure on Postgres binary
    // download, etc.), write zero-byte stub assets so the runtime
    // `include_bytes!` calls still compile. The runtime install code
    // detects the stubs and marks pgvector unavailable; memory module
    // self-disables at boot via the boot probe.
    let out_dir_path = PathBuf::from(&out_dir);
    if let Err(e) = pgvector_build::build_pgvector(&target, &out_dir_path) {
        eprintln!(
            "Warning: pgvector build failed; writing stub assets. \
             Memory features will be disabled at runtime. \
             Underlying error: {}",
            e
        );
        if let Err(stub_err) = pgvector_build::write_stubs(&out_dir_path) {
            eprintln!("ERROR: also failed to write pgvector stubs: {}", stub_err);
        }
    }

    // Generate `pgvector_assets.rs` that the runtime `include!`s. This
    // file enumerates the SQL files actually present in OUT_DIR/pgvector/sql/.
    if let Err(e) = generate_pgvector_assets(&out_dir_path) {
        eprintln!("ERROR: pgvector_assets.rs generation failed: {}", e);
    }

    println!(
        "cargo:rerun-if-changed=vendor/pgvector"
    );

    // ─────────────────────────────────────────────────────────────────
    // Hub seed — DIFFERENT FAILURE CONTRACT from every helper above.
    // Every other setup_* returns a Warning on failure and lets the
    // build continue with degraded runtime behavior. hub_seed PANICS
    // on failure: the embedded catalog is the source of truth for
    // air-gapped / first-boot users, and shipping a binary with an
    // empty or stale `binaries/hub-seed/` would silently degrade the
    // hub UI without any runtime signal. Operators on networks that
    // can't reach GitHub or Sigstore must pin `HUB_RELEASE_TAG=...`
    // and pre-stage `binaries/hub-seed/` (the skip-if-fresh path
    // consults that cache before any network call).
    // ─────────────────────────────────────────────────────────────────
    if let Err(e) = hub_seed::setup_hub_seed(&target, &binaries_dir, &out_dir) {
        panic!("Failed to fetch hub seed from GitHub: {}", e);
    }
}

/// Emit `<OUT_DIR>/pgvector_assets.rs` enumerating staged sql files.
/// The runtime crate `include!`s this file. See
/// `src/core/database/pgvector_install.rs`.
fn generate_pgvector_assets(out_dir: &std::path::Path) -> std::io::Result<()> {
    use std::fmt::Write as _;

    let staging = out_dir.join("pgvector");
    let sql_dir = staging.join("sql");

    let library_name = pgvector_build::library_filename(&env::var("TARGET").unwrap());

    let mut sql_entries: Vec<String> = Vec::new();
    if sql_dir.exists() {
        for entry in std::fs::read_dir(&sql_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() && path.extension().and_then(|e| e.to_str()) == Some("sql") {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    sql_entries.push(name.to_string());
                }
            }
        }
    }
    sql_entries.sort();

    let mut code = String::new();
    let _ = writeln!(code, "// Auto-generated by build.rs — DO NOT EDIT.");
    let _ = writeln!(
        code,
        "pub const VECTOR_LIB_FILENAME: &str = \"{}\";",
        library_name
    );
    let _ = writeln!(
        code,
        "pub const VECTOR_LIB: &[u8] = include_bytes!(concat!(env!(\"OUT_DIR\"), \"/pgvector/{}\"));",
        library_name
    );
    let _ = writeln!(
        code,
        "pub const VECTOR_CONTROL: &[u8] = include_bytes!(concat!(env!(\"OUT_DIR\"), \"/pgvector/vector.control\"));"
    );
    let _ = writeln!(code, "pub const VECTOR_SQL_FILES: &[(&str, &[u8])] = &[");
    for name in &sql_entries {
        let _ = writeln!(
            code,
            "    (\"{}\", include_bytes!(concat!(env!(\"OUT_DIR\"), \"/pgvector/sql/{}\"))),",
            name, name
        );
    }
    let _ = writeln!(code, "];");

    let dest = out_dir.join("pgvector_assets.rs");
    std::fs::write(&dest, code)?;
    Ok(())
}

fn generate_chat_repository() {
    use std::fs;
    use std::io::Write;

    let out_dir = env::var("OUT_DIR").unwrap();
    let dest_path = PathBuf::from(&out_dir).join("chat_repository.rs");

    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let modules_dir = PathBuf::from(&manifest_dir).join("src").join("modules");

    // Each `Extension` describes one sub-repository to wire into the
    // generated `ChatRepository` struct. The field name (`field`) is
    // what consumers spell as `Repos.chat.<field>` (e.g. `mcp`,
    // `memory`). The `use_path` is the qualified path the build
    // emits as a `use` statement so the type resolves.
    struct Extension {
        field: String,
        use_path: String,
        type_name: String,
    }

    let mut extensions: Vec<Extension> = Vec::new();

    // 1) In-chat extensions at `modules/chat/extensions/<name>/repository.rs`.
    //    Field name = folder name; import = chat::extensions::<name>.
    let in_chat_path = modules_dir.join("chat").join("extensions");
    if let Ok(entries) = fs::read_dir(&in_chat_path) {
        for entry in entries.flatten() {
            if let Ok(ft) = entry.file_type()
                && ft.is_dir()
            {
                let name = entry.file_name().to_string_lossy().to_string();
                let repo_file = entry.path().join("repository.rs");
                if repo_file.exists() {
                    let type_name = format!("{}ChatRepository", to_pascal_case(&name));
                    extensions.push(Extension {
                        field: name.clone(),
                        use_path: format!(
                            "crate::modules::chat::extensions::{}::{}",
                            name, type_name
                        ),
                        type_name,
                    });
                }
            }
        }
    }

    // 2) Sibling-module bridges at `modules/<sibling>/chat_extension/repository.rs`.
    //    Field name = sibling module name; import = <sibling>::chat_extension.
    //    Matches the linkme-discovery path the macros crate's build.rs uses,
    //    so a sibling bridge's repo lands in ChatRepository alongside any
    //    in-chat extension's repo with the same shape.
    if let Ok(entries) = fs::read_dir(&modules_dir) {
        for entry in entries.flatten() {
            let sibling = entry.path();
            if !sibling.is_dir() {
                continue;
            }
            let sibling_name = match sibling.file_name().and_then(|n| n.to_str()) {
                Some(n) if n != "chat" => n.to_string(),
                _ => continue,
            };
            let repo_file = sibling.join("chat_extension").join("repository.rs");
            if !repo_file.exists() {
                continue;
            }
            let type_name = format!("{}ChatRepository", to_pascal_case(&sibling_name));
            extensions.push(Extension {
                field: sibling_name.clone(),
                use_path: format!(
                    "crate::modules::{}::chat_extension::{}",
                    sibling_name, type_name
                ),
                type_name,
            });
        }
    }

    // Sort by field for consistent ordering.
    extensions.sort_by(|a, b| a.field.cmp(&b.field));

    // Generate code
    let mut code = String::from("// Auto-generated ChatRepository with extension fields\n");
    code.push_str("// DO NOT EDIT - generated by build.rs\n\n");

    for ext in &extensions {
        code.push_str(&format!("use {};\n", ext.use_path));
    }

    code.push_str("\n/// Chat repository with core and extension repositories\n");
    code.push_str("#[derive(Clone, Debug)]\n");
    code.push_str("pub struct ChatRepository {\n");
    code.push_str("    #[allow(dead_code)] // Pool is stored but not directly accessed; used to create sub-repositories\n");
    code.push_str("    pool: PgPool,\n");
    code.push_str("    pub core: ChatCoreRepository,\n");

    for ext in &extensions {
        code.push_str(&format!("    pub {}: {},\n", ext.field, ext.type_name));
    }

    code.push_str("}\n\n");
    code.push_str("impl ChatRepository {\n");
    code.push_str("    pub fn new(pool: PgPool) -> Self {\n");
    code.push_str("        Self {\n");
    code.push_str("            pool: pool.clone(),\n");
    code.push_str("            core: ChatCoreRepository::new(pool.clone()),\n");

    for ext in &extensions {
        code.push_str(&format!(
            "            {}: {}::new(pool.clone()),\n",
            ext.field, ext.type_name
        ));
    }

    code.push_str("        }\n");
    code.push_str("    }\n");
    code.push_str("}\n");

    let mut file = fs::File::create(&dest_path).unwrap();
    file.write_all(code.as_bytes()).unwrap();

    println!(
        "Generated chat repository with {} extensions",
        extensions.len()
    );
    println!("cargo:rerun-if-changed=src/modules/chat/extensions");
    println!("cargo:rerun-if-changed=src/modules");
}

fn to_pascal_case(s: &str) -> String {
    s.split('_')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
            }
        })
        .collect()
}
