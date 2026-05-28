use std::env;
use std::path::PathBuf;

#[path = "build_helper/pandoc.rs"]
mod pandoc;
#[path = "build_helper/pdfium.rs"]
mod pdfium;
#[path = "build_helper/uv.rs"]
mod uv;
#[path = "build_helper/bun.rs"]
mod bun;
#[path = "build_helper/sandbox_runtime.rs"]
mod sandbox_runtime;
#[path = "build_helper/wsl2_agent.rs"]
mod wsl2_agent;
#[path = "build_helper/pgvector.rs"]
mod pgvector_build;

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

#[tokio::main]
async fn main() {
    // Get DATABASE_URL or use local dev fallback (build-time only;
    // the dev fallback's password matches docker-compose.yaml). The
    // fallback is for ergonomics — developers shouldn't have to set
    // DATABASE_URL just to run cargo build. The string never reaches
    // the produced binary; it's used only during the build to verify
    // SQLx queries compile.
    let database_url = env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://postgres:password@127.0.0.1:54321/postgres".to_string());

    println!("cargo:rerun-if-env-changed=DATABASE_URL");

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

    // Run migrations
    let migrations_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("migrations");
    let migrator = sqlx::migrate::Migrator::new(migrations_path)
        .await
        .expect("Failed to create migrator");

    if let Err(e) = migrator.run(&pool).await {
        eprintln!("\nERROR: Migration failed: {}", e);
        panic!("Migration failed");
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

    // Find all extension repositories
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let extensions_path = PathBuf::from(&manifest_dir)
        .join("src")
        .join("modules")
        .join("chat")
        .join("extensions");

    let mut extensions = Vec::new();

    // Scan for extensions with repository.rs
    if let Ok(entries) = fs::read_dir(&extensions_path) {
        for entry in entries.flatten() {
            if let Ok(file_type) = entry.file_type()
                && file_type.is_dir()
            {
                let ext_name = entry.file_name();
                let ext_name_str = ext_name.to_string_lossy();

                // Check if repository.rs exists
                let repo_file = entry.path().join("repository.rs");
                if repo_file.exists() {
                    println!("Found extension repository: {}", ext_name_str);
                    extensions.push(ext_name_str.to_string());
                }
            }
        }
    }

    // Sort for consistent ordering
    extensions.sort();

    // Generate code
    let mut code = String::from("// Auto-generated ChatRepository with extension fields\n");
    code.push_str("// DO NOT EDIT - generated by build.rs\n\n");

    // Note: PgPool and ChatCoreRepository are imported in mod.rs
    // Extension repositories need to be imported here
    for ext in &extensions {
        let repo_type = format!("{}ChatRepository", to_pascal_case(ext));
        code.push_str(&format!(
            "use crate::modules::chat::extensions::{}::{};\n",
            ext, repo_type
        ));
    }

    code.push_str("\n/// Chat repository with core and extension repositories\n");
    code.push_str("#[derive(Clone, Debug)]\n");
    code.push_str("pub struct ChatRepository {\n");
    code.push_str("    #[allow(dead_code)] // Pool is stored but not directly accessed; used to create sub-repositories\n");
    code.push_str("    pool: PgPool,\n");
    code.push_str("    pub core: ChatCoreRepository,\n");

    for ext in &extensions {
        let repo_type = format!("{}ChatRepository", to_pascal_case(ext));
        code.push_str(&format!("    pub {}: {},\n", ext, repo_type));
    }

    code.push_str("}\n\n");
    code.push_str("impl ChatRepository {\n");
    code.push_str("    pub fn new(pool: PgPool) -> Self {\n");
    code.push_str("        Self {\n");
    code.push_str("            pool: pool.clone(),\n");
    code.push_str("            core: ChatCoreRepository::new(pool.clone()),\n");

    for ext in &extensions {
        let repo_type = format!("{}ChatRepository", to_pascal_case(ext));
        code.push_str(&format!(
            "            {}: {}::new(pool.clone()),\n",
            ext, repo_type
        ));
    }

    code.push_str("        }\n");
    code.push_str("    }\n");
    code.push_str("}\n");

    // Write to file
    let mut file = fs::File::create(&dest_path).unwrap();
    file.write_all(code.as_bytes()).unwrap();

    println!("Generated chat repository with {} extensions", extensions.len());
    println!("cargo:rerun-if-changed=src/modules/chat/extensions");
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
