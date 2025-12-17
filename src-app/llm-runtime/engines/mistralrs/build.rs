use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    let target = std::env::var("TARGET").unwrap();
    let out_dir = std::env::var("OUT_DIR").unwrap();
    let out_path = PathBuf::from(&out_dir);

    // Find mistral.rs source directory
    let source_dir = find_source_directory();

    if !source_dir.exists() {
        panic!(
            "mistral.rs source not found at: {}\n\
             Run: git submodule update --init --recursive\n\
             Or set MISTRALRS_SOURCE_PATH to a custom location",
            source_dir.display()
        );
    }

    println!("cargo:rerun-if-changed={}", source_dir.display());
    println!("cargo:rerun-if-env-changed=MISTRALRS_SOURCE_PATH");

    match build_mistralrs(&source_dir, &out_path, &target) {
        Ok(binary_path) => {
            println!("cargo:rustc-env=MISTRALRS_BINARY_PATH={}", binary_path.display());
            println!("Built mistralrs-server: {}", binary_path.display());
        }
        Err(e) => {
            panic!("Failed to build mistral.rs: {}", e);
        }
    }
}

/// Find the mistral.rs source directory
fn find_source_directory() -> PathBuf {
    // 1. Check environment variable
    if let Ok(path) = std::env::var("MISTRALRS_SOURCE_PATH") {
        return PathBuf::from(path);
    }

    // 2. Default: mistral.rs/mistralrs-server (git submodule in same directory)
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    PathBuf::from(&manifest_dir).join("mistral.rs/mistralrs-server")
}

/// Build mistralrs-server with platform-specific features
fn build_mistralrs(
    source_dir: &Path,
    out_dir: &Path,
    target: &str,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let build_dir = out_dir.join("mistralrs-build");
    let bin_dir = build_dir.join("bin");
    fs::create_dir_all(&bin_dir)?;

    let binary_name = if target.contains("windows") {
        "mistralrs-server.exe"
    } else {
        "mistralrs-server"
    };
    let target_path = bin_dir.join(binary_name);

    // Skip if already built
    if target_path.exists() {
        println!("mistralrs-server already exists at: {}", target_path.display());
        return Ok(target_path);
    }

    // Get platform-specific features
    let features = get_platform_features(target);

    // Build with cargo
    let mut cmd = Command::new("cargo");
    cmd.env("CUDA_COMPUTE_CAP", "80");
    cmd.env("RAYON_NUM_THREADS", "4");
    cmd.arg("build");
    cmd.arg("--manifest-path").arg(source_dir.join("Cargo.toml"));
    cmd.arg("--target-dir").arg(&build_dir);
    cmd.arg("--release");

    if !features.is_empty() {
        cmd.arg("--features").arg(features);
    }

    println!("Building mistralrs-server: {:?}", cmd);
    let output = cmd.output()?;

    if !output.status.success() {
        return Err(format!(
            "Cargo build failed:\n{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }

    // Find and copy the built binary
    let built_binary = build_dir.join("release").join(binary_name);
    if !built_binary.exists() {
        return Err(format!("Built binary not found at: {}", built_binary.display()).into());
    }

    fs::copy(&built_binary, &target_path)?;

    // Make executable on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&target_path)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&target_path, perms)?;
    }

    println!("Binary ready: {}", target_path.display());
    Ok(target_path)
}

/// Get platform-specific Cargo features
fn get_platform_features(target: &str) -> String {
    if target.contains("darwin") && (target.contains("aarch64") || target.contains("arm64")) {
        // macOS Apple Silicon: Metal + Accelerate
        "metal,accelerate".to_string()
    } else if target.contains("darwin") && target.contains("x86_64") {
        // macOS Intel: Accelerate
        "accelerate".to_string()
    } else if target.contains("linux") {
        // Linux: All GPU backends + CPU optimizations
        "cuda,flash-attn,cudnn".to_string()
    } else if target.contains("windows") {
        // Windows: CUDA + CPU optimizations
        "cuda,flash-attn,cudnn".to_string()
    } else {
        // Fallback
        String::new()
    }
}
