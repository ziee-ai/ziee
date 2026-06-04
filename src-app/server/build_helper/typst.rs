// Download + extract the typst binary at build time so the server can
// embed it via include_bytes! and extract on first use (mirroring the
// pandoc / pdfium / uv / bun pattern).
//
// Why typst: pandoc supports `--pdf-engine=typst` (since pandoc 3.1.7;
// our embedded pandoc is 3.7.0.2). typst handles arbitrary Unicode
// natively (≥, ≤, →, π, CJK, etc.) without the font / package
// configuration pdflatex would need, ships as a single static binary
// per target (~25–40 MB), and is itself written in Rust — clean fit
// for the self-contained-binary philosophy that motivated this swap
// away from xelatex (which would have required bundling the entire
// TeX Live distribution).

use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};

const TYPST_VERSION: &str = "0.13.1";

fn download_binary(
    url: &str,
    target_path: &Path,
    name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("Downloading {} from: {}", name, url);

    let response = ureq::get(url).call()?;
    let mut reader = response.into_body().into_reader();

    let mut file = fs::File::create(target_path)?;
    std::io::copy(&mut reader, &mut file)?;

    Ok(())
}

/// Extract the `typst` (or `typst.exe`) binary from a downloaded
/// archive into `target_dir/{target_binary_name}`. Handles both the
/// `.zip` (Windows) and `.tar.xz` (Linux/macOS) layouts typst publishes.
fn extract_typst(
    archive_path: &Path,
    target_dir: &Path,
    is_zip: bool,
    target_binary_name: &str,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    fs::create_dir_all(target_dir)?;

    if is_zip {
        // Windows: typst-{triple}.zip contains `typst-{triple}/typst.exe`.
        let file = fs::File::open(archive_path)?;
        let mut archive = zip::ZipArchive::new(file)?;

        for i in 0..archive.len() {
            let mut entry = archive.by_index(i)?;
            let filename = entry.name().to_string();

            if filename.ends_with("typst.exe") || filename.ends_with("typst") {
                let output_path = target_dir.join(target_binary_name);
                let mut outfile = fs::File::create(&output_path)?;
                std::io::copy(&mut entry, &mut outfile)?;
                return Ok(output_path);
            }
        }
    } else {
        // Linux/macOS: typst-{triple}.tar.xz → decompress to in-memory
        // tar buffer, then iterate entries looking for the `typst`
        // binary. typst's tar layout: `typst-{triple}/typst`. The
        // tarball is small enough (~25 MB compressed, ~50 MB
        // decompressed) that holding the decompressed bytes in a Vec
        // is fine — same scale the pandoc / pdfium build helpers
        // already work at.
        let mut compressed = std::io::BufReader::new(fs::File::open(archive_path)?);
        let mut decompressed: Vec<u8> = Vec::new();
        lzma_rs::xz_decompress(&mut compressed, &mut decompressed)
            .map_err(|e| format!("XZ decompression failed: {}", e))?;

        let mut archive = tar::Archive::new(Cursor::new(decompressed));

        for entry in archive.entries()? {
            let mut entry = entry?;
            let path = entry.path()?;
            let path_str = path.to_string_lossy();

            // Match `typst-{triple}/typst` exactly (avoid matching
            // bundled LICENSE / NOTICE / README files that contain the
            // word "typst" in their path).
            if path_str.ends_with("/typst") || path_str == "typst" {
                let output_path = target_dir.join(target_binary_name);
                // entry.unpack would preserve the in-archive permission
                // bits; we set 0o755 explicitly later in build.rs
                // (via fs::set_permissions) for parity with the pandoc
                // helper. Use copy to bypass unpack's path-prefix
                // safety reading-from-cwd quirk.
                let mut outfile = fs::File::create(&output_path)?;
                std::io::copy(&mut entry, &mut outfile)?;
                return Ok(output_path);
            }
        }
    }

    Err("typst binary not found in archive".into())
}

pub fn setup_typst(
    target: &str,
    target_dir: &Path,
    out_dir: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    // Dedicated typst directory under `binaries/{target}/typst/`.
    let typst_dir = target_dir.join("typst");
    fs::create_dir_all(&typst_dir)?;

    // Map cargo target triple → typst release artifact triple + extension.
    // typst publishes via GitHub Releases at
    //   https://github.com/typst/typst/releases/download/v{VERSION}/typst-{triple}.{ext}
    // Linux uses the musl variants (static-linked, run on any glibc).
    let (typst_triple, typst_extension) = if target.contains("windows") {
        if target.contains("x86_64") {
            ("x86_64-pc-windows-msvc", "zip")
        } else if target.contains("aarch64") {
            // typst doesn't publish a Windows aarch64 build; reuse
            // x86_64 (runs under WoW64 emulation). Same fallback the
            // pandoc helper uses.
            println!("Using x86_64 typst binary for Windows aarch64 target");
            ("x86_64-pc-windows-msvc", "zip")
        } else {
            panic!("Unsupported Windows architecture for typst: {}", target);
        }
    } else if target.contains("darwin") {
        if target.contains("x86_64") {
            ("x86_64-apple-darwin", "tar.xz")
        } else if target.contains("aarch64") {
            ("aarch64-apple-darwin", "tar.xz")
        } else {
            panic!("Unsupported macOS architecture for typst: {}", target);
        }
    } else if target.contains("linux") {
        if target.contains("x86_64") {
            ("x86_64-unknown-linux-musl", "tar.xz")
        } else if target.contains("aarch64") {
            ("aarch64-unknown-linux-musl", "tar.xz")
        } else {
            panic!("Unsupported Linux architecture for typst: {}", target);
        }
    } else {
        panic!("Unsupported platform for typst: {}", target);
    };

    let typst_binary_name = if target.contains("windows") {
        "typst.exe"
    } else {
        "typst"
    };

    let typst_target_path = typst_dir.join(typst_binary_name);

    if !typst_target_path.exists() {
        println!("Downloading typst binary...");

        let typst_temp_dir = Path::new(out_dir).join("typst-download");
        fs::create_dir_all(&typst_temp_dir)?;

        let typst_archive_name = format!(
            "typst-{}.{}",
            typst_triple, typst_extension,
        );
        let typst_download_url = format!(
            "https://github.com/typst/typst/releases/download/v{}/{}",
            TYPST_VERSION, typst_archive_name,
        );

        let typst_archive_path = typst_temp_dir.join(&typst_archive_name);

        if let Err(e) = download_binary(&typst_download_url, &typst_archive_path, "typst") {
            eprintln!("Warning: Failed to download typst: {}", e);
            return Err(e);
        }

        let extracted_path = extract_typst(
            &typst_archive_path,
            &typst_temp_dir,
            typst_extension == "zip",
            typst_binary_name,
        )?;

        fs::copy(&extracted_path, &typst_target_path)?;
        println!("Successfully installed typst to {:?}", typst_target_path);

        // Make executable on Unix. The runtime extract path
        // (embedded.rs) does this again — we set it here so the
        // download path is invokable standalone for testing.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&typst_target_path)?.permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&typst_target_path, perms)?;
        }

        // Best-effort temp cleanup; failure here doesn't break the build.
        let _ = fs::remove_dir_all(&typst_temp_dir);
    } else {
        println!("typst binary already exists at {:?}", typst_target_path);
    }

    println!("typst binary ready for embedding at {:?}", typst_target_path);

    Ok(())
}
