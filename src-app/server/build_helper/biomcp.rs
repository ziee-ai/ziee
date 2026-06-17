// BioMCP binary download + verification for embedding.
//
// Vendors the genomoncology/biomcp single-binary release and stages it
// under binaries/{target}/biomcp/ for the runtime `include_bytes!`
// (see src/modules/bio_mcp/embedded.rs).
//
// FAIL-SOFT (mirrors pgvector, NOT pandoc/uv/bun): biomcp is an OPTIONAL
// feature. On any failure (no network, missing asset, sha256 mismatch,
// extraction error) a ZERO-BYTE STUB is written to the staging path so
// the runtime `include_bytes!` still compiles; `bio_mcp::embedded::
// biomcp_available()` then returns false and the module self-disables at
// boot. A biomcp fetch failure must never break the whole server build.
//
// Pin / override: `BIOMCP_VERSION` (default below) and `BIOMCP_GITHUB_REPO`
// (default `genomoncology/biomcp`) — same testability/air-gap seam shape
// as hub_seed's env knobs.

use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;

/// Pinned upstream release (the helper prepends `v` → tag `v0.8.23`).
/// Override at build time with `BIOMCP_VERSION=0.x.y`.
const BIOMCP_VERSION: &str = "0.8.23";

pub fn setup_biomcp(
    target: &str,
    target_dir: &Path,
    _out_dir: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("=== SETUP_BIOMCP CALLED ===");
    println!("Target: {}", target);

    let biomcp_dir = target_dir.join("biomcp");
    let binary_name = if target.contains("windows") {
        "biomcp.exe"
    } else {
        "biomcp"
    };
    let binary_path = biomcp_dir.join(binary_name);
    // Records the version of the currently-staged binary (like hub_seed's
    // `.tag`). The staging dir persists across `cargo clean`, so without this
    // a bumped `BIOMCP_VERSION` would silently reuse the OLD binary.
    let version_file = biomcp_dir.join(".version");

    let version = std::env::var("BIOMCP_VERSION").unwrap_or_else(|_| BIOMCP_VERSION.to_string());
    let repo =
        std::env::var("BIOMCP_GITHUB_REPO").unwrap_or_else(|_| "genomoncology/biomcp".to_string());

    // Skip ONLY if a real (non-empty) binary of the CURRENT version is already
    // staged. A zero-byte stub from a prior failed build, or a binary staged
    // for a different version, is re-fetched.
    let staged_ok = binary_path.exists()
        && fs::metadata(&binary_path).map(|m| m.len() > 0).unwrap_or(false)
        && fs::read_to_string(&version_file)
            .map(|v| v.trim() == version)
            .unwrap_or(false);
    if staged_ok {
        println!("BioMCP {} already staged at: {:?}", version, binary_path);
        return Ok(());
    }

    // Map Rust target triple → biomcp release asset name.
    // IMPORTANT: keep this triple set in sync with the `#[cfg(...)]` arms in
    // `src/modules/bio_mcp/embedded.rs` (and `mcp/utils/embedded.rs`). A
    // mismatch would either break the build (helper skips a supported triple,
    // embedded.rs `include_bytes!` finds no file) or stage a binary embedded.rs
    // never references.
    let asset = match target {
        "x86_64-unknown-linux-gnu" | "x86_64-unknown-linux-musl" => "biomcp-linux-x86_64.tar.gz",
        "aarch64-unknown-linux-gnu" | "aarch64-unknown-linux-musl" => "biomcp-linux-arm64.tar.gz",
        "x86_64-apple-darwin" => "biomcp-darwin-x86_64.tar.gz",
        "aarch64-apple-darwin" => "biomcp-darwin-arm64.tar.gz",
        "x86_64-pc-windows-msvc" => "biomcp-windows-x86_64.zip",
        _ => {
            // Unsupported triple: the runtime embedded.rs `compile_error!`
            // (same triple set as uv/bun) fires before `include_bytes!`
            // is ever referenced, so no stub is needed here.
            eprintln!("Warning: BioMCP binary not available for target: {}", target);
            return Ok(());
        }
    };

    let download_url = format!(
        "https://github.com/{}/releases/download/v{}/{}",
        repo, version, asset
    );

    println!("Downloading BioMCP {} for {}", version, target);
    println!("URL: {}", download_url);

    if let Err(e) = fs::create_dir_all(&biomcp_dir) {
        eprintln!("Warning: cannot create BioMCP staging dir: {}", e);
        return write_stub(&binary_path);
    }

    let archive_path = biomcp_dir.join(asset);

    // Download + verify the optional sha256 sidecar.
    if let Err(e) = download_and_verify(&download_url, &archive_path) {
        eprintln!(
            "Warning: BioMCP download/verify failed ({}); writing stub. \
             Biomedical features will self-disable at runtime.",
            e
        );
        let _ = fs::remove_file(&archive_path);
        return write_stub(&binary_path);
    }

    // Extract the single `biomcp` binary out of the archive.
    let extracted = if target.contains("windows") {
        extract_zip(&archive_path, &biomcp_dir, binary_name)
    } else {
        extract_tar_gz(&archive_path, &biomcp_dir, binary_name)
    };
    let _ = fs::remove_file(&archive_path);

    if let Err(e) = extracted {
        eprintln!("Warning: BioMCP extraction failed ({}); writing stub.", e);
        return write_stub(&binary_path);
    }
    if !binary_path.exists()
        || fs::metadata(&binary_path).map(|m| m.len() == 0).unwrap_or(true)
    {
        eprintln!("Warning: BioMCP binary not found after extraction; writing stub.");
        return write_stub(&binary_path);
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&binary_path)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&binary_path, perms)?;
    }

    // Record the staged version so a later `BIOMCP_VERSION` bump re-fetches.
    let _ = fs::write(&version_file, &version);

    println!("BioMCP {} binary ready at: {:?}", version, binary_path);
    Ok(())
}

/// Write a zero-byte stub at the staging path so the runtime
/// `include_bytes!` compiles. The runtime treats an empty payload as
/// "biomcp unavailable" and self-disables.
fn write_stub(binary_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(parent) = binary_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::File::create(binary_path)?; // zero bytes
    println!("BioMCP: wrote zero-byte stub at {:?} (feature self-disables)", binary_path);
    Ok(())
}

fn download_and_verify(url: &str, dest: &Path) -> Result<(), Box<dyn std::error::Error>> {
    // biomcp single-binary archives are tens of MB; cap at 300 MB.
    let bytes = http_get_bytes(url, 300 * 1024 * 1024)?;

    // sha256 verification is MANDATORY: biomcp ships a `<asset>.sha256` for
    // every release artifact (verified for all 5 platforms). A missing,
    // malformed, or mismatching sidecar FAILS the download — the caller then
    // stages a stub and the feature self-disables, rather than embedding an
    // unverified binary (supply-chain defense).
    let sidecar = http_get_bytes(&format!("{}.sha256", url), 1024 * 1024).map_err(|e| {
        format!(
            "sha256 sidecar unavailable ({}): refusing to embed an unverified binary",
            e
        )
    })?;
    let text = String::from_utf8_lossy(&sidecar);
    let expected = text
        .split_whitespace()
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    if expected.len() != 64 || !expected.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(format!("sha256 sidecar malformed (not 64 hex chars): {:?}", expected).into());
    }
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    let actual: String = hasher
        .finalize()
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect();
    if actual != expected {
        return Err(format!("sha256 mismatch: expected {}, got {}", expected, actual).into());
    }
    println!("BioMCP sha256 verified.");

    fs::write(dest, &bytes)?;
    Ok(())
}

fn http_get_bytes(url: &str, limit: usize) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let response = ureq::get(url).call()?;
    if response.status() != 200 {
        return Err(format!("GET {} -> status {}", url, response.status()).into());
    }
    let bytes = response
        .into_body()
        .with_config()
        .limit(limit as u64)
        .read_to_vec()?;
    Ok(bytes)
}

fn extract_tar_gz(
    archive: &Path,
    dest_dir: &Path,
    binary_name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    use flate2::read::GzDecoder;
    use tar::Archive;

    let file = fs::File::open(archive)?;
    let decoder = GzDecoder::new(file);
    let mut archive = Archive::new(decoder);

    // The archive may place the binary at top level or under a dir;
    // match on the file name (mirrors uv.rs).
    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = entry.path()?.to_path_buf();
        if path.file_name().and_then(|n| n.to_str()) == Some(binary_name) {
            entry.unpack(dest_dir.join(binary_name))?;
            println!("Extracted BioMCP binary from: {:?}", path);
            break;
        }
    }
    Ok(())
}

fn extract_zip(
    archive: &Path,
    dest_dir: &Path,
    binary_name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    use zip::ZipArchive;

    let file = fs::File::open(archive)?;
    let mut archive = ZipArchive::new(file)?;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let name = file.name().to_string();
        if name.ends_with(binary_name) || name.ends_with(&format!("/{}", binary_name)) {
            let mut dest_file = fs::File::create(dest_dir.join(binary_name))?;
            std::io::copy(&mut file, &mut dest_file)?;
            println!("Extracted BioMCP binary from: {}", name);
            break;
        }
    }
    Ok(())
}
