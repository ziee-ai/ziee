use std::fs;
use std::path::{Path, PathBuf};

const PANDOC_VERSION: &str = "3.7.0.2";

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

fn extract_pandoc(
    archive_path: &Path,
    target_dir: &Path,
    is_zip: bool,
    target_binary_name: &str,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    fs::create_dir_all(target_dir)?;

    if is_zip {
        // Extract ZIP file
        let file = fs::File::open(archive_path)?;
        let mut archive = zip::ZipArchive::new(file)?;

        for i in 0..archive.len() {
            let mut file = archive.by_index(i)?;
            let filename = file.name();

            // Look for pandoc or pandoc.exe (may be in bin/ directory or root)
            if filename.ends_with("pandoc")
                || filename.ends_with("pandoc.exe")
                || filename.ends_with("bin/pandoc")
            {
                let output_path = target_dir.join(target_binary_name);

                let mut outfile = fs::File::create(&output_path)?;
                std::io::copy(&mut file, &mut outfile)?;

                return Ok(output_path);
            }
        }
    } else {
        // Extract tar.gz file
        let tar_gz = fs::File::open(archive_path)?;
        let tar = flate2::read::GzDecoder::new(tar_gz);
        let mut archive = tar::Archive::new(tar);

        for entry in archive.entries()? {
            let mut entry = entry?;
            let path = entry.path()?;
            let path_str = path.to_string_lossy();

            // Look for pandoc (may be in bin/ directory or root)
            if path_str.ends_with("pandoc")
                || path_str.ends_with("pandoc.exe")
                || path_str.ends_with("bin/pandoc")
            {
                let output_path = target_dir.join(target_binary_name);
                entry.unpack(&output_path)?;
                return Ok(output_path);
            }
        }
    }

    Err("Pandoc binary not found in archive".into())
}

pub fn setup_pandoc(
    target: &str,
    target_dir: &Path,
    out_dir: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    // Use dedicated Pandoc directory
    let pandoc_dir = target_dir.join("pandoc");
    fs::create_dir_all(&pandoc_dir)?;

    // Map target to Pandoc platform names
    let (pandoc_platform, pandoc_arch, pandoc_extension) = if target.contains("windows") {
        if target.contains("x86_64") {
            ("windows", "x86_64", "zip")
        } else if target.contains("aarch64") {
            println!("Using x86_64 Pandoc binary for Windows aarch64 target");
            ("windows", "x86_64", "zip")
        } else {
            panic!("Unsupported Windows architecture for Pandoc: {}", target);
        }
    } else if target.contains("darwin") {
        if target.contains("x86_64") {
            ("x86_64", "macOS", "zip")
        } else if target.contains("aarch64") {
            ("arm64", "macOS", "zip")
        } else {
            panic!("Unsupported macOS architecture for Pandoc: {}", target);
        }
    } else if target.contains("linux") {
        if target.contains("x86_64") {
            ("linux", "amd64", "tar.gz")
        } else if target.contains("aarch64") {
            ("linux", "arm64", "tar.gz")
        } else {
            panic!("Unsupported Linux architecture for Pandoc: {}", target);
        }
    } else {
        panic!("Unsupported platform for Pandoc: {}", target);
    };

    let pandoc_binary_name = if target.contains("windows") {
        "pandoc.exe"
    } else {
        "pandoc"
    };

    let pandoc_target_path = pandoc_dir.join(pandoc_binary_name);

    // Download Pandoc if it doesn't exist
    if !pandoc_target_path.exists() {
        println!("Downloading Pandoc binary...");

        let pandoc_temp_dir = Path::new(out_dir).join("pandoc-download");
        fs::create_dir_all(&pandoc_temp_dir)?;

        let pandoc_archive_name = format!(
            "pandoc-{}-{}-{}.{}",
            PANDOC_VERSION, pandoc_platform, pandoc_arch, pandoc_extension
        );
        let pandoc_download_url = format!(
            "https://github.com/jgm/pandoc/releases/download/{}/{}",
            PANDOC_VERSION, pandoc_archive_name
        );

        let pandoc_archive_path = pandoc_temp_dir.join(&pandoc_archive_name);

        // Download the Pandoc archive
        if let Err(e) = download_binary(&pandoc_download_url, &pandoc_archive_path, "Pandoc") {
            eprintln!("Warning: Failed to download Pandoc: {}", e);
            return Err(e);
        }

        // Extract the Pandoc binary
        let extracted_path = extract_pandoc(
            &pandoc_archive_path,
            &pandoc_temp_dir,
            pandoc_extension == "zip",
            pandoc_binary_name,
        )?;

        // Copy to target directory
        fs::copy(&extracted_path, &pandoc_target_path)?;
        println!("Successfully installed Pandoc to {:?}", pandoc_target_path);

        // Make it executable on Unix
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&pandoc_target_path)?.permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&pandoc_target_path, perms)?;
        }

        // Clean up temporary files
        fs::remove_dir_all(&pandoc_temp_dir).ok();
    } else {
        println!("Pandoc binary already exists at {:?}", pandoc_target_path);
    }

    // Binary is now ready for embedding at pandoc_target_path
    // No need to copy to bin directories - will be embedded via include-flate
    println!("Pandoc binary ready for embedding at {:?}", pandoc_target_path);

    Ok(())
}
