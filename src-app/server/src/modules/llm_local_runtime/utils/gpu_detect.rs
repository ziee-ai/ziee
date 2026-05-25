// GPU backend detection for LLM runtime
// Detects available GPU acceleration: CUDA (NVIDIA), ROCm (AMD), Metal (Apple Silicon)

use std::process::Command;

/// Resolve a binary by name to its absolute path, searching only the
/// trusted system directories (NOT $PATH). Closes
/// 08-llm-local-runtime F-14 (Low): the previous `Command::new("nvidia-smi")`
/// inherits the server's PATH, so a directory at the front of PATH
/// containing a malicious `nvidia-smi` shadows the real one. Returns
/// None when the binary isn't in any trusted dir; callers skip the
/// detection step in that case.
fn resolve_system_binary(name: &str) -> Option<std::path::PathBuf> {
    // Vendor-specific tools live under these well-known prefixes. We
    // prefer absolute paths so PATH injection / DLL search-order
    // attacks can't pivot through GPU detection.
    const TRUSTED_DIRS: &[&str] = &[
        // Linux distros
        "/usr/bin",
        "/usr/sbin",
        "/usr/local/bin",
        // CUDA / ROCm typical install
        "/usr/local/cuda/bin",
        "/opt/rocm/bin",
        // macOS
        "/usr/local/bin",
        "/opt/homebrew/bin",
        "/System/Library",
    ];
    for dir in TRUSTED_DIRS {
        let candidate = std::path::PathBuf::from(dir).join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
        // macOS sometimes has system_profiler under /usr/sbin/
        let alt = std::path::PathBuf::from(dir).join("usr").join("sbin").join(name);
        if alt.is_file() {
            return Some(alt);
        }
    }
    None
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuBackend {
    Cpu,
    Cuda,
    Metal,
    Rocm,
}

impl GpuBackend {
    pub fn as_str(&self) -> &'static str {
        match self {
            GpuBackend::Cpu => "cpu",
            GpuBackend::Cuda => "cuda",
            GpuBackend::Metal => "metal",
            GpuBackend::Rocm => "rocm",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "cpu" => Some(GpuBackend::Cpu),
            "cuda" => Some(GpuBackend::Cuda),
            "metal" => Some(GpuBackend::Metal),
            "rocm" => Some(GpuBackend::Rocm),
            _ => None,
        }
    }
}

/// Detect the best available GPU backend for the current system
/// Priority: CUDA > Metal > ROCm > CPU
pub fn detect_gpu_backend() -> GpuBackend {
    // Check for NVIDIA CUDA
    if is_cuda_available() {
        tracing::info!("Detected NVIDIA GPU (CUDA available)");
        return GpuBackend::Cuda;
    }

    // Check for Apple Metal (macOS only)
    #[cfg(target_os = "macos")]
    {
        if is_metal_available() {
            tracing::info!("Detected Apple GPU (Metal available)");
            return GpuBackend::Metal;
        }
    }

    // Check for AMD ROCm
    if is_rocm_available() {
        tracing::info!("Detected AMD GPU (ROCm available)");
        return GpuBackend::Rocm;
    }

    // Fallback to CPU
    tracing::info!("No GPU acceleration detected, using CPU backend");
    GpuBackend::Cpu
}

fn is_cuda_available() -> bool {
    // Try nvidia-smi command (absolute-path resolved, no PATH lookup).
    // Closes 08-llm-local-runtime F-14 (Low). If the binary is not in
    // any trusted dir we skip this probe and fall through to the
    // library-existence check below.
    if let Some(nvidia_smi) = resolve_system_binary("nvidia-smi")
        && let Ok(output) = Command::new(nvidia_smi).output()
            && output.status.success() {
                tracing::debug!("nvidia-smi command succeeded");
                return true;
            }

    // Try checking for CUDA libraries (Linux)
    #[cfg(target_os = "linux")]
    {
        if std::path::Path::new("/usr/local/cuda/lib64/libcudart.so").exists()
            || std::path::Path::new("/usr/lib/x86_64-linux-gnu/libcudart.so").exists()
        {
            tracing::debug!("Found CUDA libraries in system");
            return true;
        }
    }

    false
}

fn is_metal_available() -> bool {
    // Metal is available on all modern macOS with Apple Silicon or modern Intel GPUs
    #[cfg(target_os = "macos")]
    {
        // Check architecture - Apple Silicon always has Metal
        #[cfg(target_arch = "aarch64")]
        {
            tracing::debug!("Running on Apple Silicon (Metal supported)");
            return true;
        }

        // For Intel Macs, try to check via system_profiler
        #[cfg(target_arch = "x86_64")]
        {
            if let Some(system_profiler) = resolve_system_binary("system_profiler") {
                if let Ok(output) = Command::new(system_profiler)
                    .arg("SPDisplaysDataType")
                    .output()
                {
                    if output.status.success() {
                        let stdout = String::from_utf8_lossy(&output.stdout);
                        // Metal is supported on macOS 10.11+ with compatible GPUs
                        if stdout.contains("Metal") {
                            tracing::debug!("Metal support detected via system_profiler");
                            return true;
                        }
                    }
                }
            }

            // Assume Metal available on modern Intel Macs (macOS 10.15+)
            tracing::debug!("Assuming Metal support on Intel Mac");
            return true;
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        false
    }
}

fn is_rocm_available() -> bool {
    // Try rocm-smi command (absolute-path resolved, no PATH lookup)
    if let Some(rocm_smi) = resolve_system_binary("rocm-smi")
        && let Ok(output) = Command::new(rocm_smi).output()
            && output.status.success() {
                tracing::debug!("rocm-smi command succeeded");
                return true;
            }

    // Try checking for ROCm libraries (Linux)
    #[cfg(target_os = "linux")]
    {
        if std::path::Path::new("/opt/rocm/lib/libamdhip64.so").exists()
            || std::path::Path::new("/opt/rocm/hip/lib/libamdhip64.so").exists()
        {
            tracing::debug!("Found ROCm libraries in system");
            return true;
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gpu_backend_as_str() {
        assert_eq!(GpuBackend::Cpu.as_str(), "cpu");
        assert_eq!(GpuBackend::Cuda.as_str(), "cuda");
        assert_eq!(GpuBackend::Metal.as_str(), "metal");
        assert_eq!(GpuBackend::Rocm.as_str(), "rocm");
    }

    #[test]
    fn test_gpu_backend_from_str() {
        assert_eq!(GpuBackend::from_str("cpu"), Some(GpuBackend::Cpu));
        assert_eq!(GpuBackend::from_str("CPU"), Some(GpuBackend::Cpu));
        assert_eq!(GpuBackend::from_str("cuda"), Some(GpuBackend::Cuda));
        assert_eq!(GpuBackend::from_str("CUDA"), Some(GpuBackend::Cuda));
        assert_eq!(GpuBackend::from_str("metal"), Some(GpuBackend::Metal));
        assert_eq!(GpuBackend::from_str("rocm"), Some(GpuBackend::Rocm));
        assert_eq!(GpuBackend::from_str("invalid"), None);
    }

    #[test]
    fn test_detect_gpu_backend_returns_some_backend() {
        // Should always return a valid backend (at minimum CPU)
        let backend = detect_gpu_backend();
        assert!(matches!(
            backend,
            GpuBackend::Cpu | GpuBackend::Cuda | GpuBackend::Metal | GpuBackend::Rocm
        ));
    }
}
