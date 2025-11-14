use std::fs;
use std::path::{Path, PathBuf};

fn download_binary(
    url: &str,
    target_path: &Path,
    name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("Downloading {} from: {}", name, url);

    let response = ureq::get(url).call()?;
    let mut reader = response.into_reader();

    let mut file = fs::File::create(target_path)?;
    std::io::copy(&mut reader, &mut file)?;

    Ok(())
}

fn extract_pdfium(
    archive_path: &Path,
    target_dir: &Path,
    target_binary_name: &str,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    fs::create_dir_all(target_dir)?;

    // Extract tar.gz file
    let tar_gz = fs::File::open(archive_path)?;
    let tar = flate2::read::GzDecoder::new(tar_gz);
    let mut archive = tar::Archive::new(tar);

    // PDFium dynamic libraries are typically in lib/ directory
    let library_names = if target_binary_name.contains("windows") {
        vec!["bin/pdfium.dll", "lib/pdfium.dll"]
    } else if target_binary_name.contains("darwin") {
        vec!["lib/libpdfium.dylib"]
    } else {
        vec!["lib/libpdfium.so"]
    };

    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = entry.path()?;
        let path_str = path.to_string_lossy();

        // Check if this is the PDFium library we're looking for
        if library_names.iter().any(|name| path_str.ends_with(name)) {
            let output_path = target_dir.join(target_binary_name);
            entry.unpack(&output_path)?;
            return Ok(output_path);
        }
    }

    Err("PDFium library not found in archive".into())
}

pub fn setup_pdfium(
    target: &str,
    target_dir: &Path,
    out_dir: &str,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    // Use dedicated PDFium directory
    let pdfium_dir = target_dir.join("pdfium");
    fs::create_dir_all(&pdfium_dir)?;

    // Map target to PDFium platform names
    let (pdfium_platform, pdfium_arch) = if target.contains("windows") {
        if target.contains("x86_64") {
            ("win", "x64")
        } else if target.contains("aarch64") {
            ("win", "arm64")
        } else {
            panic!("Unsupported Windows architecture for PDFium: {}", target);
        }
    } else if target.contains("darwin") {
        if target.contains("x86_64") {
            ("mac", "x64")
        } else if target.contains("aarch64") {
            ("mac", "arm64")
        } else {
            panic!("Unsupported macOS architecture for PDFium: {}", target);
        }
    } else if target.contains("linux") {
        if target.contains("x86_64") {
            ("linux", "x64")
        } else if target.contains("aarch64") {
            ("linux", "arm64")
        } else {
            panic!("Unsupported Linux architecture for PDFium: {}", target);
        }
    } else {
        panic!("Unsupported platform for PDFium: {}", target);
    };

    // Use simple platform-specific naming (not target triple)
    // This matches what embedded.rs expects in include_bytes!
    let pdfium_binary_name = if target.contains("windows") {
        "pdfium.dll".to_string()
    } else if target.contains("darwin") {
        "libpdfium.dylib".to_string()
    } else {
        "libpdfium.so".to_string()
    };

    let pdfium_target_path = pdfium_dir.join(&pdfium_binary_name);

    // Download PDFium if it doesn't exist
    if !pdfium_target_path.exists() {
        println!("Downloading PDFium library...");

        let pdfium_temp_dir = Path::new(out_dir).join("pdfium-download");
        fs::create_dir_all(&pdfium_temp_dir)?;

        let pdfium_archive_name = format!("pdfium-{}-{}.tgz", pdfium_platform, pdfium_arch);
        let pdfium_download_url = format!(
            "https://github.com/bblanchon/pdfium-binaries/releases/latest/download/{}",
            pdfium_archive_name
        );

        let pdfium_archive_path = pdfium_temp_dir.join(&pdfium_archive_name);

        // Download the PDFium archive
        if let Err(e) = download_binary(&pdfium_download_url, &pdfium_archive_path, "PDFium") {
            eprintln!("Warning: Failed to download PDFium: {}", e);
            return Err(e);
        }

        // Extract the PDFium library
        let extracted_path = extract_pdfium(&pdfium_archive_path, &pdfium_temp_dir, &pdfium_binary_name)?;

        // Copy to target directory
        fs::copy(&extracted_path, &pdfium_target_path)?;
        println!("Successfully installed PDFium to {:?}", pdfium_target_path);

        // Make it executable on Unix
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&pdfium_target_path)?.permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&pdfium_target_path, perms)?;
        }

        // Clean up temporary files
        fs::remove_dir_all(&pdfium_temp_dir).ok();
    } else {
        println!("PDFium binary already exists at {:?}", pdfium_target_path);
    }

    // Binary is now ready for embedding at pdfium_target_path
    // No need to copy to lib directories - will be embedded via include-flate
    println!("PDFium binary ready for embedding at {:?}", pdfium_target_path);

    Ok(pdfium_target_path)
}
