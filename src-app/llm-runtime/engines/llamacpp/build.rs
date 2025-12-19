use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    let target = std::env::var("TARGET").unwrap();
    let out_dir = std::env::var("OUT_DIR").unwrap();
    let out_path = PathBuf::from(&out_dir);

    // Default source path - expect llama.cpp source in src-engines/llama.cpp
    let source_dir = find_source_directory();

    if !source_dir.exists() {
        panic!(
            "llama.cpp source not found at: {}\n\
             Run: git submodule update --init --recursive\n\
             Or set LLAMACPP_SOURCE_PATH to a custom location",
            source_dir.display()
        );
    }

    println!("cargo:rerun-if-changed={}", source_dir.display());
    println!("cargo:rerun-if-env-changed=LLAMACPP_SOURCE_PATH");

    match build_llamacpp(&source_dir, &out_path, &target) {
        Ok(binary_path) => {
            println!("cargo:rustc-env=LLAMACPP_BINARY_PATH={}", binary_path.display());
            println!("Built llama-server: {}", binary_path.display());
        }
        Err(e) => {
            panic!("Failed to build llama.cpp: {}", e);
        }
    }
}

/// Find the llama.cpp source directory
fn find_source_directory() -> PathBuf {
    // 1. Check environment variable
    if let Ok(path) = std::env::var("LLAMACPP_SOURCE_PATH") {
        return PathBuf::from(path);
    }

    // 2. Default: llama.cpp (git submodule in same directory)
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    PathBuf::from(&manifest_dir).join("llama.cpp")
}

/// Build llama.cpp with comprehensive platform and backend support
fn build_llamacpp(
    source_dir: &Path,
    out_dir: &Path,
    target: &str,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let build_dir = out_dir.join("llamacpp-build");
    let install_dir = out_dir.join("llamacpp");
    let bin_dir = install_dir.join("bin");

    fs::create_dir_all(&build_dir)?;
    fs::create_dir_all(&install_dir)?;
    fs::create_dir_all(&bin_dir)?;

    // Check if binary already exists (CACHING)
    let binary_name = if target.contains("windows") {
        "llama-server.exe"
    } else {
        "llama-server"
    };
    let build_bin_dir = build_dir.join("bin");
    let cached_binary = build_bin_dir.join(binary_name);

    if cached_binary.exists() {
        println!("Using cached llama-server binary: {}", cached_binary.display());
        return Ok(cached_binary);
    }

    // Get platform and backend configuration
    let (platform_flags, backend_flags) = get_cmake_configuration(target);

    // Configure CMake
    let mut cmake_cmd = Command::new("cmake");
    cmake_cmd.current_dir(source_dir);
    cmake_cmd.arg("-B").arg(&build_dir);
    cmake_cmd.arg("-S").arg(".");
    cmake_cmd.arg("-DCMAKE_BUILD_TYPE=Release");
    cmake_cmd.arg(format!("-DCMAKE_INSTALL_PREFIX={}", install_dir.display()));

    // Platform-specific RPATH configuration
    configure_rpath(&mut cmake_cmd, target);

    // Add all configuration flags
    for (key, value) in platform_flags.iter().chain(backend_flags.iter()) {
        cmake_cmd.arg(format!("-D{}={}", key, value));
    }

    // Common llama.cpp build settings
    cmake_cmd.arg("-DLLAMA_BUILD_TESTS=OFF");
    cmake_cmd.arg("-DLLAMA_BUILD_EXAMPLES=OFF");
    cmake_cmd.arg("-DLLAMA_BUILD_SERVER=ON");
    cmake_cmd.arg("-DLLAMA_BUILD_LLAMA_CLI=OFF");
    cmake_cmd.arg("-DLLAMA_CURL=OFF"); // Avoid dependency issues

    // Comprehensive binary disabling - only build llama-server
    cmake_cmd.arg("-DLLAMA_BUILD_LLAMA_RUN=OFF");
    cmake_cmd.arg("-DLLAMA_BUILD_LLAMA_BENCH=OFF");
    cmake_cmd.arg("-DLLAMA_BUILD_LLAMA_QUANTIZE=OFF");
    cmake_cmd.arg("-DLLAMA_BUILD_LLAMA_PERPLEXITY=OFF");
    cmake_cmd.arg("-DLLAMA_BUILD_LLAMA_BATCHED_BENCH=OFF");
    cmake_cmd.arg("-DLLAMA_BUILD_LLAMA_TTS=OFF");
    cmake_cmd.arg("-DLLAMA_BUILD_LLAMA_GGUF_SPLIT=OFF");
    cmake_cmd.arg("-DLLAMA_BUILD_LLAMA_IMATRIX=OFF");
    cmake_cmd.arg("-DLLAMA_BUILD_LLAMA_TOKENIZE=OFF");
    cmake_cmd.arg("-DLLAMA_BUILD_LLAMA_MTMD_CLI=OFF");

    // Install paths
    cmake_cmd.arg("-DCMAKE_INSTALL_BINDIR=bin");
    cmake_cmd.arg("-DCMAKE_INSTALL_LIBDIR=lib");

    println!("Running CMake configure: {:?}", cmake_cmd);
    let output = cmake_cmd.output()?;
    if !output.status.success() {
        return Err(format!(
            "CMake configure failed:\n{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }

    // Build
    let mut build_cmd = Command::new("cmake");
    build_cmd.arg("--build").arg(&build_dir);
    build_cmd.arg("--config").arg("Release");
    if let Ok(cores) = std::thread::available_parallelism() {
        build_cmd.arg("-j").arg(cores.get().to_string());
    }

    println!("Running CMake build: {:?}", build_cmd);
    let output = build_cmd.output()?;
    if !output.status.success() {
        return Err(format!(
            "CMake build failed:\n{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }

    // Use binary directly from build directory (RPATH is already configured correctly)
    // Skip install/consolidate as it doesn't work reliably - build dir has proper RPATH
    let build_bin_dir = build_dir.join("bin");
    let binary_path = build_bin_dir.join(binary_name);

    if !binary_path.exists() {
        return Err(format!("Binary not found after build: {}", binary_path.display()).into());
    }

    // Make executable on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&binary_path)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&binary_path, perms)?;
    }

    println!("Binary ready: {}", binary_path.display());
    Ok(binary_path)
}

/// Get comprehensive CMake configuration for platform and backends
fn get_cmake_configuration(target: &str) -> (HashMap<String, String>, HashMap<String, String>) {
    let mut platform_flags = HashMap::new();
    let mut backend_flags = HashMap::new();

    // Common CPU settings
    backend_flags.insert("GGML_CPU".to_string(), "ON".to_string());
    backend_flags.insert("GGML_CPU_ALL_VARIANTS".to_string(), "ON".to_string());
    backend_flags.insert("GGML_NATIVE".to_string(), "OFF".to_string());
    backend_flags.insert("GGML_BACKEND_DL".to_string(), "ON".to_string());

    if target.contains("darwin") || target.contains("apple") {
        configure_macos(&mut platform_flags, &mut backend_flags, target);
    } else if target.contains("linux") {
        configure_linux(&mut platform_flags, &mut backend_flags, target);
    } else if target.contains("windows") {
        configure_windows(&mut platform_flags, &mut backend_flags, target);
    }

    (platform_flags, backend_flags)
}

/// Configure macOS-specific settings
fn configure_macos(
    platform_flags: &mut HashMap<String, String>,
    backend_flags: &mut HashMap<String, String>,
    target: &str,
) {
    if target.contains("aarch64") || target.contains("arm64") {
        // Apple Silicon: Metal + Accelerate
        platform_flags.insert("CMAKE_OSX_ARCHITECTURES".to_string(), "arm64".to_string());
        backend_flags.insert("GGML_METAL".to_string(), "ON".to_string());
        backend_flags.insert("GGML_METAL_USE_BF16".to_string(), "ON".to_string());
        backend_flags.insert("GGML_METAL_EMBED_LIBRARY".to_string(), "ON".to_string());
        backend_flags.insert("GGML_ACCELERATE".to_string(), "ON".to_string());
    } else {
        // Intel Mac: Accelerate + SIMD
        platform_flags.insert("CMAKE_OSX_ARCHITECTURES".to_string(), "x86_64".to_string());
        backend_flags.insert("GGML_ACCELERATE".to_string(), "ON".to_string());
        backend_flags.insert("GGML_AVX2".to_string(), "ON".to_string());
        backend_flags.insert("GGML_AVX".to_string(), "ON".to_string());
        backend_flags.insert("GGML_METAL".to_string(), "OFF".to_string());
    }

    // Disable other GPU backends on macOS
    backend_flags.insert("GGML_CUDA".to_string(), "OFF".to_string());
    backend_flags.insert("GGML_VULKAN".to_string(), "OFF".to_string());
    backend_flags.insert("GGML_OPENCL".to_string(), "OFF".to_string());
}

/// Configure Linux-specific settings with multi-backend support
fn configure_linux(
    _platform_flags: &mut HashMap<String, String>,
    backend_flags: &mut HashMap<String, String>,
    _target: &str,
) {
    // Enable SIMD optimizations
    backend_flags.insert("GGML_AVX2".to_string(), "ON".to_string());
    backend_flags.insert("GGML_AVX".to_string(), "ON".to_string());
    backend_flags.insert("GGML_SSE3".to_string(), "ON".to_string());

    // CRITICAL: Disable OpenMP to avoid Intel MKL dependency (libmtmd.so)
    backend_flags.insert("GGML_OPENMP".to_string(), "OFF".to_string());

    // Disable BLAS - use SIMD optimizations instead
    backend_flags.insert("GGML_BLAS".to_string(), "OFF".to_string());

    // Enable GPU backends (some may be optional if dependencies not found)
    // CUDA - try to enable, CMake will skip if not found
    backend_flags.insert("GGML_CUDA".to_string(), "ON".to_string());
    backend_flags.insert("CMAKE_CUDA_ARCHITECTURES".to_string(), "52;61;70;75;80;86;89;90".to_string());

    // Vulkan and OpenCL are optional - disable by default to avoid build failures
    // Users can enable via environment variables if they have the SDKs
    backend_flags.insert("GGML_VULKAN".to_string(), "OFF".to_string());
    backend_flags.insert("GGML_OPENCL".to_string(), "OFF".to_string());
    backend_flags.insert("GGML_METAL".to_string(), "OFF".to_string());
}

/// Configure Windows-specific settings
fn configure_windows(
    platform_flags: &mut HashMap<String, String>,
    backend_flags: &mut HashMap<String, String>,
    _target: &str,
) {
    // Windows-specific build settings
    platform_flags.insert("BUILD_SHARED_LIBS".to_string(), "ON".to_string());

    // Enable GPU backends
    backend_flags.insert("GGML_CUDA".to_string(), "ON".to_string());
    backend_flags.insert("CMAKE_CUDA_ARCHITECTURES".to_string(), "52;61;70;75;80;86;89;90".to_string());
    backend_flags.insert("GGML_VULKAN".to_string(), "ON".to_string());
    backend_flags.insert("GGML_OPENCL".to_string(), "ON".to_string());
    backend_flags.insert("GGML_METAL".to_string(), "OFF".to_string());
}

/// Configure RPATH for runtime library loading
fn configure_rpath(cmake_cmd: &mut Command, target: &str) {
    if target.contains("darwin") || target.contains("apple") {
        // macOS: Use @loader_path
        cmake_cmd.arg("-DCMAKE_BUILD_RPATH=@loader_path");
        cmake_cmd.arg("-DCMAKE_INSTALL_RPATH=@loader_path");
        cmake_cmd.arg("-DCMAKE_BUILD_WITH_INSTALL_RPATH=ON");
        cmake_cmd.arg("-DCMAKE_INSTALL_RPATH_USE_LINK_PATH=OFF");
    } else if target.contains("linux") {
        // Linux: Use $ORIGIN
        cmake_cmd.arg("-DCMAKE_BUILD_RPATH=$ORIGIN");
        cmake_cmd.arg("-DCMAKE_INSTALL_RPATH=$ORIGIN");
        cmake_cmd.arg("-DCMAKE_BUILD_WITH_INSTALL_RPATH=ON");
        cmake_cmd.arg("-DCMAKE_INSTALL_RPATH_USE_LINK_PATH=OFF");
    }
    // Windows: DLL search includes executable directory by default
}

/// Consolidate all runtime files into bin/ directory
fn consolidate_runtime_files(
    install_dir: &Path,
    bin_dir: &Path,
    target: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    fs::create_dir_all(bin_dir)?;

    // Move libraries from lib/ to bin/
    let lib_dir = install_dir.join("lib");
    if lib_dir.exists() {
        for entry in fs::read_dir(&lib_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                let file_name = path.file_name().unwrap();
                let dest = bin_dir.join(file_name);
                if !dest.exists() {
                    fs::rename(&path, &dest)?;
                    println!("Moved to bin/: {:?}", file_name);
                }
            }
        }
        let _ = fs::remove_dir_all(&lib_dir);
    }

    // Move Metal resources on macOS
    if target.contains("darwin") || target.contains("apple") {
        for entry in fs::read_dir(install_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                let file_name = path.file_name().unwrap().to_string_lossy();
                if file_name.ends_with(".metallib") || file_name.ends_with(".metal") {
                    let dest = bin_dir.join(file_name.as_ref());
                    if !dest.exists() {
                        fs::rename(&path, &dest)?;
                        println!("Moved Metal resource: {}", file_name);
                    }
                }
            }
        }
    }

    // Clean up unnecessary files in bin/
    for entry in fs::read_dir(bin_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() {
            let file_name = path.file_name().unwrap().to_string_lossy();

            // Keep llama-server and libraries/resources
            if file_name == "llama-server" || file_name == "llama-server.exe" {
                continue;
            }
            if file_name.ends_with(".dylib")
                || file_name.ends_with(".so")
                || file_name.ends_with(".dll")
                || file_name.ends_with(".metallib")
                || file_name.ends_with(".metal")
            {
                continue;
            }

            // Remove other executables
            if !file_name.contains('.') || path.metadata()?.permissions().readonly() == false {
                let _ = fs::remove_file(&path);
                println!("Removed unnecessary file: {}", file_name);
            }
        }
    }

    Ok(())
}
