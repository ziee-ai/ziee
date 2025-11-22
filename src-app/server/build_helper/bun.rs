// Bun binary download and extraction for embedding
// Downloads Bun binaries from GitHub releases for all supported platforms

use std::fs;
use std::path::Path;

const BUN_VERSION: &str = "1.1.38";

pub fn setup_bun(target: &str, target_dir: &Path, _out_dir: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("=== SETUP_BUN CALLED ===");
    println!("Target: {}", target);
    println!("Target dir: {:?}", target_dir);

    let bun_dir = target_dir.join("bun");

    // Check if Bun binary already exists
    let binary_name = if target.contains("windows") { "bun.exe" } else { "bun" };
    let binary_path = bun_dir.join(binary_name);

    if binary_path.exists() {
        println!("Bun binary already exists at: {:?}", binary_path);
        return Ok(());
    }

    println!("Bun binary does NOT exist, will download...");

    // Map Rust target triple to Bun platform name
    let platform = match target {
        "x86_64-unknown-linux-gnu" | "x86_64-unknown-linux-musl" =>
            "linux-x64",
        "aarch64-unknown-linux-gnu" | "aarch64-unknown-linux-musl" =>
            "linux-aarch64",
        "x86_64-apple-darwin" =>
            "darwin-x64",
        "aarch64-apple-darwin" =>
            "darwin-aarch64",
        "x86_64-pc-windows-msvc" =>
            "windows-x64",
        _ => {
            eprintln!("Warning: Bun binaries not available for target: {}", target);
            eprintln!("Please install Bun manually: https://bun.sh/");
            return Ok(());
        }
    };

    let archive_name = format!("bun-{}.zip", platform);
    let download_url = format!(
        "https://github.com/oven-sh/bun/releases/download/bun-v{}/{}",
        BUN_VERSION, archive_name
    );

    println!("Downloading Bun {} for {}", BUN_VERSION, target);
    println!("URL: {}", download_url);

    // Create target directory
    fs::create_dir_all(&bun_dir)?;

    // Download archive
    let archive_path = bun_dir.join(&archive_name);
    download_file(&download_url, &archive_path)?;

    // Extract binary
    extract_zip(&archive_path, &bun_dir, binary_name)?;

    // Verify extraction
    if !binary_path.exists() {
        return Err(format!("Bun binary not found after extraction: {:?}", binary_path).into());
    }

    println!("Bun binary ready at: {:?}", binary_path);

    // Clean up archive
    let _ = fs::remove_file(&archive_path);

    Ok(())
}

fn download_file(url: &str, dest: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let response = ureq::get(url).call()?;

    if response.status() != 200 {
        return Err(format!("Download failed with status: {}", response.status()).into());
    }

    // Bun binary is ~93MB, set limit to 150MB
    let bytes = response.into_body()
        .with_config()
        .limit(150 * 1024 * 1024)
        .read_to_vec()?;
    fs::write(dest, &bytes)?;

    Ok(())
}

fn extract_zip(archive: &Path, dest_dir: &Path, binary_name: &str) -> Result<(), Box<dyn std::error::Error>> {
    use zip::ZipArchive;

    let file = fs::File::open(archive)?;
    let mut archive = ZipArchive::new(file)?;

    // Bun archives contain a directory structure, find the binary
    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let name = file.name().to_string();

        // Look for bun or bun.exe in the archive
        if name.ends_with(binary_name) || name.ends_with(&format!("/{}", binary_name)) {
            let dest_path = dest_dir.join(binary_name);
            let mut dest_file = fs::File::create(&dest_path)?;
            std::io::copy(&mut file, &mut dest_file)?;
            println!("Extracted Bun binary from: {}", name);
            break;
        }
    }

    Ok(())
}
