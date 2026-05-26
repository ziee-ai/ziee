//! pgvector extension build helper.
//!
//! Downloads matching Postgres 18.3.0 binaries from theseus-rs, builds
//! the vendored pgvector source via `make`, and stages the resulting
//! `vector.{so|dylib|dll}` + `vector.control` + `sql/vector--*.sql`
//! into `OUT_DIR/pgvector/` so the server can embed them via
//! `include_bytes!` and write them into the embedded-PG install dir
//! at runtime.
//!
//! Ported from
//! /home/pbya/projects/ziee-chat-ref/build-helpers/src/pgvector.rs but
//! simplified — we only target the current build host, not 5 triples.
//! macOS SDK-path wrapper retained verbatim (it fixes a known
//! pg_config bug on Apple Silicon).

#![allow(dead_code)]

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Build pgvector for the current target. Returns `Ok(())` on success.
/// On failure, the caller should write stub files to OUT_DIR so the
/// downstream `include_bytes!` calls still compile (memory will then
/// fail-soft at runtime via the boot probe).
pub fn build_pgvector(target: &str, out_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let pgvector_src = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("vendor")
        .join("pgvector");

    if !pgvector_src.exists() {
        return Err(format!(
            "pgvector source not found at {} — did you run `git submodule update --init`?",
            pgvector_src.display()
        )
        .into());
    }

    let staging = out_dir.join("pgvector");
    fs::create_dir_all(&staging)?;

    let library_name = library_filename(target);
    let staging_lib = staging.join(library_name);

    // Skip rebuild if artifacts already present (incremental builds).
    if staging_lib.exists()
        && staging.join("vector.control").exists()
        && staging.join("sql").exists()
    {
        return Ok(());
    }

    // 1. Download matching Postgres binaries.
    let postgres_dir = setup_postgresql_binaries(&pgvector_src, target)?;

    // 2. Run `make` (or `nmake` on Windows).
    build_pgvector_extension(&pgvector_src, &postgres_dir, target)?;

    // 3. Copy outputs into OUT_DIR/pgvector/.
    let built_lib = pgvector_src.join(library_name);
    if !built_lib.exists() {
        return Err(format!(
            "pgvector library not found after build: {}",
            built_lib.display()
        )
        .into());
    }
    fs::copy(&built_lib, &staging_lib)?;

    let control_src = pgvector_src.join("vector.control");
    if !control_src.exists() {
        return Err("pgvector vector.control not found".into());
    }
    fs::copy(&control_src, staging.join("vector.control"))?;

    let sql_src = pgvector_src.join("sql");
    let sql_dst = staging.join("sql");
    fs::create_dir_all(&sql_dst)?;
    for entry in fs::read_dir(&sql_src)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() && path.extension().map_or(false, |e| e == "sql") {
            fs::copy(&path, sql_dst.join(path.file_name().unwrap()))?;
        }
    }

    Ok(())
}

/// Write empty stubs so `include_bytes!` in the runtime crate compiles
/// even when pgvector build failed. The runtime install code treats
/// zero-length payloads as "no pgvector available" and falls back to
/// fail-soft mode (memory module marks itself disabled at boot).
pub fn write_stubs(out_dir: &Path) -> std::io::Result<()> {
    let staging = out_dir.join("pgvector");
    fs::create_dir_all(staging.join("sql"))?;

    // Write a zero-byte library + zero-byte control + a single empty sql file.
    fs::File::create(staging.join(stub_lib_filename()))?.write_all(&[])?;
    fs::File::create(staging.join("vector.control"))?.write_all(&[])?;
    fs::File::create(staging.join("sql").join("vector--stub.sql"))?.write_all(&[])?;
    Ok(())
}

/// Library filename for the target triple.
pub fn library_filename(target: &str) -> &'static str {
    if target.contains("windows") {
        "vector.dll"
    } else if target.contains("darwin") {
        "vector.dylib"
    } else {
        "vector.so"
    }
}

/// Library filename for the current host (used by `write_stubs`).
fn stub_lib_filename() -> &'static str {
    if cfg!(target_os = "windows") {
        "vector.dll"
    } else if cfg!(target_os = "macos") {
        "vector.dylib"
    } else {
        "vector.so"
    }
}

/// Download + extract Postgres 18.3.0 binaries from theseus-rs into
/// pgvector_src.join("postgresql-18.3.0"). Skips if already present.
fn setup_postgresql_binaries(
    pgvector_dir: &Path,
    target: &str,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let postgres_dir = pgvector_dir.join("postgresql-18.3.0");
    if postgres_dir.exists() && postgres_dir.join("bin").exists() {
        return Ok(postgres_dir);
    }

    let pkg = postgres_package_for_target(target)
        .ok_or_else(|| format!("unsupported target for theseus-rs binaries: {target}"))?;
    let url = format!(
        "https://github.com/theseus-rs/postgresql-binaries/releases/download/18.3.0/{pkg}"
    );

    let archive_path = pgvector_dir.join(pkg);
    download_file(&url, &archive_path)?;
    extract_archive(&archive_path, &postgres_dir)?;
    let _ = fs::remove_file(&archive_path);
    Ok(postgres_dir)
}

fn postgres_package_for_target(target: &str) -> Option<&'static str> {
    match () {
        _ if target.contains("aarch64") && target.contains("apple") => {
            Some("postgresql-18.3.0-aarch64-apple-darwin.tar.gz")
        }
        _ if target.contains("x86_64") && target.contains("apple") => {
            Some("postgresql-18.3.0-x86_64-apple-darwin.tar.gz")
        }
        _ if target.contains("x86_64") && target.contains("linux") => {
            Some("postgresql-18.3.0-x86_64-unknown-linux-gnu.tar.gz")
        }
        _ if target.contains("aarch64") && target.contains("linux") => {
            Some("postgresql-18.3.0-aarch64-unknown-linux-gnu.tar.gz")
        }
        _ if target.contains("x86_64") && target.contains("windows") => {
            Some("postgresql-18.3.0-x86_64-pc-windows-msvc.zip")
        }
        _ => None,
    }
}

fn build_pgvector_extension(
    pgvector_dir: &Path,
    postgres_dir: &Path,
    target: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let pg_config_exe = if target.contains("windows") {
        "pg_config.exe"
    } else {
        "pg_config"
    };
    let pg_config_path = postgres_dir.join("bin").join(pg_config_exe);

    let make_cmd = if target.contains("windows") { "nmake" } else { "make" };

    let mut cmd = Command::new(make_cmd);
    cmd.current_dir(pgvector_dir).env("PG_CONFIG", &pg_config_path);

    if target.contains("darwin") {
        if target.contains("aarch64") || target.contains("arm64") {
            cmd.env("OPTFLAGS", "");
        }
        // Wire xcrun SDK + pg_config wrapper to fix Apple Silicon SDK
        // paths. Verbatim from the reference (Lines 227-321) — see
        // /home/pbya/projects/ziee-chat-ref/build-helpers/src/pgvector.rs.
        apply_macos_sdk_wrapper(pgvector_dir, &pg_config_path, &mut cmd)?;
    } else if target.contains("windows") {
        cmd.args(["/f", "Makefile.win"]);
        let pgroot = postgres_dir.display().to_string();
        let pgroot_clean = if let Some(stripped) = pgroot.strip_prefix(r"\\?\") {
            stripped.to_string()
        } else {
            pgroot
        };
        cmd.env("PGROOT", pgroot_clean);
    } else if target.contains("powerpc") || target.contains("ppc64") {
        cmd.env("OPTFLAGS", "");
    }

    cmd.env("enable_debug", "no");
    cmd.env("ENABLE_DEBUG", "no");
    cmd.env("DEBUG", "");
    cmd.env("PROFILE", "");

    let output = cmd.output()?;
    if !output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        eprintln!("pgvector build STDOUT:\n{stdout}");
        eprintln!("pgvector build STDERR:\n{stderr}");
        return Err(format!(
            "pgvector make failed with exit code: {:?}",
            output.status.code()
        )
        .into());
    }
    Ok(())
}

fn apply_macos_sdk_wrapper(
    pgvector_dir: &Path,
    pg_config_path: &Path,
    cmd: &mut Command,
) -> Result<(), Box<dyn std::error::Error>> {
    let sdk_output = Command::new("xcrun")
        .args(["--sdk", "macosx", "--show-sdk-path"])
        .output()?;
    if !sdk_output.status.success() {
        return Ok(()); // xcrun unavailable — let plain make try
    }
    let sdk_path = String::from_utf8_lossy(&sdk_output.stdout).trim().to_string();

    let wrapper_dir = pgvector_dir.join("pg_config_wrapper");
    fs::create_dir_all(&wrapper_dir)?;
    let wrapper_script = wrapper_dir.join("pg_config");
    let original = pg_config_path.display();
    let wrapper_content = format!(
        r#"#!/bin/bash
case "$1" in
    --cppflags)
        echo "-isysroot {sdk_path} -I/opt/homebrew/opt/icu4c/include -I/opt/homebrew/opt/openssl/include"
        ;;
    --cflags)
        "{original}" "$@" | sed 's|-isysroot /Library/Developer/CommandLineTools/SDKs/MacOSX[0-9]*\.[0-9]*\.sdk|-isysroot {sdk_path}|g'
        ;;
    *)
        "{original}" "$@" | sed 's|/Library/Developer/CommandLineTools/SDKs/MacOSX[0-9]*\.[0-9]*\.sdk|{sdk_path}|g'
        ;;
esac
"#,
    );
    fs::write(&wrapper_script, wrapper_content)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&wrapper_script)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&wrapper_script, perms)?;
    }
    cmd.env("PG_CONFIG", &wrapper_script);
    cmd.env(
        "PATH",
        format!(
            "{}:{}",
            wrapper_dir.display(),
            std::env::var("PATH").unwrap_or_default()
        ),
    );
    cmd.env("PG_CPPFLAGS", format!("-isysroot {sdk_path}"));
    cmd.env("PG_CFLAGS", format!("-isysroot {sdk_path}"));
    cmd.env("PG_LDFLAGS", format!("-isysroot {sdk_path}"));
    Ok(())
}

fn download_file(url: &str, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let response = ureq::get(url).call()?;
    let mut reader = response.into_body().into_reader();
    let mut file = fs::File::create(path)?;
    std::io::copy(&mut reader, &mut file)?;
    Ok(())
}

fn extract_archive(
    archive_path: &Path,
    extract_to: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    fs::create_dir_all(extract_to)?;
    let is_zip = archive_path.extension().and_then(|s| s.to_str()) == Some("zip");

    if is_zip {
        let file = fs::File::open(archive_path)?;
        let mut archive = zip::ZipArchive::new(file)?;
        let mut prefix: Option<String> = None;
        for i in 0..archive.len() {
            let f = archive.by_index(i)?;
            if let Some(p) = f.enclosed_name() {
                if let Some(first) = p.to_string_lossy().split('/').next() {
                    prefix.get_or_insert_with(|| first.to_string());
                }
            }
        }
        for i in 0..archive.len() {
            let mut entry = archive.by_index(i)?;
            let entry_path = match entry.enclosed_name() {
                Some(p) => p.to_path_buf(),
                None => continue,
            };
            let rel = if let Some(ref pfx) = prefix {
                entry_path.strip_prefix(pfx).unwrap_or(&entry_path).to_path_buf()
            } else {
                entry_path.clone()
            };
            let outpath = extract_to.join(&rel);
            if entry.name().ends_with('/') {
                fs::create_dir_all(&outpath)?;
            } else {
                if let Some(parent) = outpath.parent() {
                    fs::create_dir_all(parent)?;
                }
                let mut outfile = fs::File::create(&outpath)?;
                std::io::copy(&mut entry, &mut outfile)?;
            }
        }
    } else {
        let temp_dir = extract_to.parent().unwrap().join("temp_postgres_extract");
        fs::create_dir_all(&temp_dir)?;
        let f = fs::File::open(archive_path)?;
        let gz = flate2::read::GzDecoder::new(f);
        let mut archive = tar::Archive::new(gz);
        archive.unpack(&temp_dir)?;
        for entry in fs::read_dir(&temp_dir)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                move_dir_contents(&entry.path(), extract_to)?;
                break;
            }
        }
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let pg_config_path = extract_to.join("bin").join("pg_config");
        if pg_config_path.exists() {
            let mut perms = fs::metadata(&pg_config_path)?.permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&pg_config_path, perms)?;
        }
    }
    Ok(())
}

fn move_dir_contents(src: &Path, dst: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if src_path.is_dir() {
            move_dir_contents(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}
