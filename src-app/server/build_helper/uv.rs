// UV binary download and extraction for embedding
// Downloads UV binaries from GitHub releases for all supported platforms

use std::fs;
use std::path::Path;

const UV_VERSION: &str = "0.5.20";

pub fn setup_uv(target: &str, target_dir: &Path, _out_dir: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("=== SETUP_UV CALLED ===");
    println!("Target: {}", target);
    println!("Target dir: {:?}", target_dir);

    let uv_dir = target_dir.join("uv");

    // Check if UV binary already exists
    let binary_name = if target.contains("windows") { "uv.exe" } else { "uv" };
    let binary_path = uv_dir.join(binary_name);

    if binary_path.exists() {
        println!("UV binary already exists at: {:?}", binary_path);
        return Ok(());
    }

    println!("UV binary does NOT exist, will download...");

    // Map Rust target triple to UV platform name
    let platform = match target {
        "x86_64-unknown-linux-gnu" | "x86_64-unknown-linux-musl" =>
            "x86_64-unknown-linux-gnu",
        "aarch64-unknown-linux-gnu" | "aarch64-unknown-linux-musl" =>
            "aarch64-unknown-linux-gnu",
        "x86_64-apple-darwin" =>
            "x86_64-apple-darwin",
        "aarch64-apple-darwin" =>
            "aarch64-apple-darwin",
        "x86_64-pc-windows-msvc" =>
            "x86_64-pc-windows-msvc",
        _ => {
            eprintln!("Warning: UV binaries not available for target: {}", target);
            eprintln!("Please install UV manually: https://github.com/astral-sh/uv");
            return Ok(());
        }
    };

    let archive_ext = if target.contains("windows") { "zip" } else { "tar.gz" };
    let archive_name = format!("uv-{}.{}", platform, archive_ext);
    let download_url = format!(
        "https://github.com/astral-sh/uv/releases/download/{}/{}",
        UV_VERSION, archive_name
    );

    println!("Downloading UV {} for {}", UV_VERSION, target);
    println!("URL: {}", download_url);

    // Create target directory
    fs::create_dir_all(&uv_dir)?;

    // Download archive
    let archive_path = uv_dir.join(&archive_name);
    download_file(&download_url, &archive_path)?;

    // Extract binary
    if target.contains("windows") {
        extract_zip(&archive_path, &uv_dir)?;
    } else {
        extract_tar_gz(&archive_path, &uv_dir)?;
    }

    // Verify extraction
    if !binary_path.exists() {
        return Err(format!("UV binary not found after extraction: {:?}", binary_path).into());
    }

    println!("UV binary ready at: {:?}", binary_path);

    // Clean up archive
    let _ = fs::remove_file(&archive_path);

    Ok(())
}

fn download_file(url: &str, dest: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let response = ureq::get(url).call()?;

    if response.status() != 200 {
        return Err(format!("Download failed with status: {}", response.status()).into());
    }

    // UV binary is ~37MB, set limit to 100MB
    let bytes = response.into_body()
        .with_config()
        .limit(100 * 1024 * 1024)
        .read_to_vec()?;
    fs::write(dest, &bytes)?;

    Ok(())
}

fn extract_tar_gz(archive: &Path, dest_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    use flate2::read::GzDecoder;
    use tar::Archive;

    let file = fs::File::open(archive)?;
    let decoder = GzDecoder::new(file);
    let mut archive = Archive::new(decoder);

    // UV archives contain a top-level directory, extract the binary from it
    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = entry.path()?.to_path_buf();

        // Look for the "uv" binary in any directory
        if path.file_name().and_then(|n| n.to_str()) == Some("uv") {
            entry.unpack(dest_dir.join("uv"))?;
            println!("Extracted UV binary from: {:?}", path);
            break;
        }
    }

    Ok(())
}

fn extract_zip(archive: &Path, dest_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    use zip::ZipArchive;

    let file = fs::File::open(archive)?;
    let mut archive = ZipArchive::new(file)?;

    // Find and extract uv.exe
    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let name = file.name().to_string();

        if name.ends_with("uv.exe") {
            let mut dest_file = fs::File::create(dest_dir.join("uv.exe"))?;
            std::io::copy(&mut file, &mut dest_file)?;
            println!("Extracted UV binary from: {}", name);
            break;
        }
    }

    Ok(())
}
