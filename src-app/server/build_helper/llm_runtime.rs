// LLM Runtime binary download and extraction for embedding
// Downloads llama-server and mistralrs-server binaries from GitHub releases
// Supports multiple GPU backends: CPU-only, CUDA, ROCm, Metal

use std::fs;
use std::path::Path;

// TODO: Update these versions when releases are available
const LLAMA_SERVER_VERSION: &str = "b4313";  // llama.cpp commit/tag
const MISTRALRS_VERSION: &str = "0.4.3";

// TODO: Update these URLs to point to actual release repositories
// Assuming separate repos: ziee-team/llama-server-releases and ziee-team/mistralrs-server-releases
const LLAMA_SERVER_REPO: &str = "ziee-team/llama-server-releases";
const MISTRALRS_REPO: &str = "ziee-team/mistralrs-server-releases";

pub fn setup_llm_runtime(
    target: &str,
    target_dir: &Path,
    _out_dir: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("=== SETUP_LLM_RUNTIME CALLED ===");
    println!("Target: {}", target);
    println!("Target dir: {:?}", target_dir);

    // Detect which GPU backends are available for this platform
    let backends = get_backends_for_target(target);

    println!("GPU backends for {}: {:?}", target, backends);

    // Setup llama-server for all applicable backends
    for backend in &backends {
        if let Err(e) = setup_llama_server(target, target_dir, backend) {
            eprintln!("Warning: Failed to setup llama-server ({}): {}", backend, e);
        }
    }

    // Setup mistralrs-server for all applicable backends
    for backend in &backends {
        if let Err(e) = setup_mistralrs_server(target, target_dir, backend) {
            eprintln!("Warning: Failed to setup mistralrs-server ({}): {}", backend, e);
        }
    }

    Ok(())
}

fn get_backends_for_target(target: &str) -> Vec<&'static str> {
    let mut backends = vec!["cpu"]; // CPU always available

    if target.contains("x86_64") && target.contains("linux") {
        // Linux x86_64: CPU, CUDA, ROCm
        backends.push("cuda");
        backends.push("rocm");
    } else if target.contains("x86_64") && target.contains("windows") {
        // Windows x86_64: CPU, CUDA
        backends.push("cuda");
    } else if target.contains("aarch64") && target.contains("darwin") {
        // macOS ARM64: Metal (included in base binary)
        backends.push("metal");
    }
    // Other platforms: CPU only

    backends
}

fn setup_llama_server(
    target: &str,
    target_dir: &Path,
    backend: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let backend_dir = target_dir.join("llm-runtime").join(backend);
    let binary_name = get_binary_name("llama-server", backend, target);
    let binary_path = backend_dir.join(&binary_name);

    // Check if already exists
    if binary_path.exists() {
        println!("llama-server ({}) already exists at: {:?}", backend, binary_path);
        return Ok(());
    }

    println!("llama-server ({}) does NOT exist, will download...", backend);

    // Create backend directory
    fs::create_dir_all(&backend_dir)?;

    // Map target to platform/arch for download URL
    let (platform, arch) = map_target_to_platform_arch(target)?;

    // Build download URL
    // Format: https://github.com/ziee-team/llama-server-releases/releases/download/v{version}/llama-server-{platform}-{arch}-{backend}.tar.gz
    let archive_ext = if target.contains("windows") { "zip" } else { "tar.gz" };
    let archive_name = format!("llama-server-{}-{}-{}.{}", platform, arch, backend, archive_ext);
    let download_url = format!(
        "https://github.com/{}/releases/download/v{}/{}",
        LLAMA_SERVER_REPO, LLAMA_SERVER_VERSION, archive_name
    );

    println!("Downloading llama-server {} ({}) for {}", LLAMA_SERVER_VERSION, backend, target);
    println!("URL: {}", download_url);

    // Download archive
    let archive_path = backend_dir.join(&archive_name);
    download_file(&download_url, &archive_path)?;

    // Extract binary
    if target.contains("windows") {
        extract_zip(&archive_path, &backend_dir, &binary_name)?;
    } else {
        extract_tar_gz(&archive_path, &backend_dir, &binary_name)?;
    }

    // Verify extraction
    if !binary_path.exists() {
        return Err(format!("llama-server binary not found after extraction: {:?}", binary_path).into());
    }

    // Create version file
    let version_path = backend_dir.join("version.txt");
    fs::write(&version_path, format!("llama-server\n{}\n", LLAMA_SERVER_VERSION))?;

    println!("llama-server ({}) ready at: {:?}", backend, binary_path);

    // Clean up archive
    let _ = fs::remove_file(&archive_path);

    Ok(())
}

fn setup_mistralrs_server(
    target: &str,
    target_dir: &Path,
    backend: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let backend_dir = target_dir.join("llm-runtime").join(backend);
    let binary_name = get_binary_name("mistralrs-server", backend, target);
    let binary_path = backend_dir.join(&binary_name);

    // Check if already exists
    if binary_path.exists() {
        println!("mistralrs-server ({}) already exists at: {:?}", backend, binary_path);
        return Ok(());
    }

    println!("mistralrs-server ({}) does NOT exist, will download...", backend);

    // Create backend directory
    fs::create_dir_all(&backend_dir)?;

    // Map target to platform/arch for download URL
    let (platform, arch) = map_target_to_platform_arch(target)?;

    // Build download URL
    let archive_ext = if target.contains("windows") { "zip" } else { "tar.gz" };
    let archive_name = format!("mistralrs-server-{}-{}-{}.{}", platform, arch, backend, archive_ext);
    let download_url = format!(
        "https://github.com/{}/releases/download/v{}/{}",
        MISTRALRS_REPO, MISTRALRS_VERSION, archive_name
    );

    println!("Downloading mistralrs-server {} ({}) for {}", MISTRALRS_VERSION, backend, target);
    println!("URL: {}", download_url);

    // Download archive
    let archive_path = backend_dir.join(&archive_name);
    download_file(&download_url, &archive_path)?;

    // Extract binary
    if target.contains("windows") {
        extract_zip(&archive_path, &backend_dir, &binary_name)?;
    } else {
        extract_tar_gz(&archive_path, &backend_dir, &binary_name)?;
    }

    // Verify extraction
    if !binary_path.exists() {
        return Err(format!("mistralrs-server binary not found after extraction: {:?}", binary_path).into());
    }

    // Append to version file (llama-server may have already created it)
    let version_path = backend_dir.join("version.txt");
    let version_content = if version_path.exists() {
        format!("{}\nmistralrs-server\n{}\n", fs::read_to_string(&version_path)?, MISTRALRS_VERSION)
    } else {
        format!("mistralrs-server\n{}\n", MISTRALRS_VERSION)
    };
    fs::write(&version_path, version_content)?;

    println!("mistralrs-server ({}) ready at: {:?}", backend, binary_path);

    // Clean up archive
    let _ = fs::remove_file(&archive_path);

    Ok(())
}

fn get_binary_name(base_name: &str, backend: &str, target: &str) -> String {
    let extension = if target.contains("windows") { ".exe" } else { "" };

    // For non-CPU backends, append backend suffix
    if backend == "cpu" {
        format!("{}{}", base_name, extension)
    } else {
        format!("{}-{}{}", base_name, backend, extension)
    }
}

fn map_target_to_platform_arch(target: &str) -> Result<(&'static str, &'static str), Box<dyn std::error::Error>> {
    let (platform, arch) = match target {
        "x86_64-unknown-linux-gnu" | "x86_64-unknown-linux-musl" =>
            ("linux", "x86_64"),
        "aarch64-unknown-linux-gnu" | "aarch64-unknown-linux-musl" =>
            ("linux", "aarch64"),
        "x86_64-apple-darwin" =>
            ("macos", "x86_64"),
        "aarch64-apple-darwin" =>
            ("macos", "aarch64"),
        "x86_64-pc-windows-msvc" =>
            ("windows", "x86_64"),
        _ => {
            return Err(format!(
                "LLM runtime binaries not available for target: {}. \
                Supported: Linux (x86_64, aarch64), macOS (x86_64, aarch64), Windows (x86_64)",
                target
            ).into());
        }
    };

    Ok((platform, arch))
}

fn download_file(url: &str, dest: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let response = ureq::get(url).call()?;

    if response.status() != 200 {
        return Err(format!("Download failed with status: {}", response.status()).into());
    }

    // LLM binaries can be large (50-200MB), set limit to 500MB
    let bytes = response.into_body()
        .with_config()
        .limit(500 * 1024 * 1024)
        .read_to_vec()?;
    fs::write(dest, &bytes)?;

    Ok(())
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

    // Look for the binary in the archive
    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = entry.path()?.to_path_buf();

        // Look for binary by name
        if path.file_name().and_then(|n| n.to_str()) == Some(binary_name) {
            let dest_path = dest_dir.join(binary_name);
            entry.unpack(&dest_path)?;
            println!("Extracted {} from: {:?}", binary_name, path);

            // Set executable permissions on Unix
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mut perms = fs::metadata(&dest_path)?.permissions();
                perms.set_mode(0o755);
                fs::set_permissions(&dest_path, perms)?;
            }

            return Ok(());
        }
    }

    Err(format!("Binary {} not found in archive", binary_name).into())
}

fn extract_zip(
    archive: &Path,
    dest_dir: &Path,
    binary_name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    use zip::ZipArchive;

    let file = fs::File::open(archive)?;
    let mut archive = ZipArchive::new(file)?;

    // Find and extract binary
    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let name = file.name().to_string();

        if name.ends_with(binary_name) {
            let dest_path = dest_dir.join(binary_name);
            let mut dest_file = fs::File::create(&dest_path)?;
            std::io::copy(&mut file, &mut dest_file)?;
            println!("Extracted {} from: {}", binary_name, name);
            return Ok(());
        }
    }

    Err(format!("Binary {} not found in archive", binary_name).into())
}
