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
            if let Ok(file_type) = entry.file_type() {
                if file_type.is_dir() {
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
