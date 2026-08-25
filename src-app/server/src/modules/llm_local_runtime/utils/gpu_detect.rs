// GPU backend detection for LLM runtime
// Detects available GPU acceleration: CUDA (NVIDIA), ROCm (AMD), Metal (Apple Silicon)

use std::process::Command;
use std::sync::OnceLock;
use std::time::Duration;

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

/// Hard cap on how long a single host/GPU probe subprocess may run. A cold
/// `nvidia-smi` can take tens of seconds (driver/GPU init) — and with no cap a
/// slow probe stalls the whole `/detect-gpu` handler, so the proxy in front of
/// it returns 502 and the settings-page GPU card never renders. We'd rather
/// treat a probe that won't answer in a few seconds as "unavailable" and fall
/// through to the cheap library-existence checks.
const PROBE_TIMEOUT: Duration = Duration::from_secs(3);

/// Run a resolved binary and capture its output, abandoning the wait after
/// `timeout`. Returns None on spawn error or timeout. On timeout the worker
/// thread + its child are detached (not killed) — the child is a read-only
/// vendor probe that exits on its own shortly after; we just stop waiting.
fn probe_command_with_timeout(
    bin: std::path::PathBuf,
    args: &[&str],
    timeout: Duration,
) -> Option<std::process::Output> {
    let owned: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(Command::new(bin).args(&owned).output());
    });
    match rx.recv_timeout(timeout) {
        Ok(Ok(output)) => Some(output),
        // spawn error, sender dropped, or timed out → caller treats as "no signal"
        _ => None,
    }
}

/// Resolve a trusted system binary then run it under [`PROBE_TIMEOUT`].
fn probe_trusted(name: &str, args: &[&str]) -> Option<std::process::Output> {
    let bin = resolve_system_binary(name)?;
    probe_command_with_timeout(bin, args, PROBE_TIMEOUT)
}

/// Run a trusted system binary (absolute-path resolved, no `$PATH` lookup)
/// and capture stdout, bounded by [`PROBE_TIMEOUT`]. Used for runtime host
/// probing (`uname`/`sysctl`).
fn run_trusted(name: &str, args: &[&str]) -> Option<String> {
    let out = probe_trusted(name, args)?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// The OS family the process is **actually running on**, probed at runtime
/// (`uname -s`) rather than read from the compile-time target. Maps to the
/// release-artifact platform token (`linux`/`macos`/`windows`).
///
/// Runtime detection matters because "what host am I on" must not be coupled
/// to "what target was I built for" (e.g. a binary run under emulation, or a
/// universal build). On Windows there is no `uname`; a Windows binary only
/// runs on Windows, so the compile-time constant is the correct fallback.
pub fn host_platform() -> String {
    // Memoized: the host OS is stable for the process lifetime, so the `uname`
    // spawn runs once (not on every detect-gpu / check-updates call).
    static CACHE: OnceLock<String> = OnceLock::new();
    CACHE
        .get_or_init(|| {
            if let Some(uname) = run_trusted("uname", &["-s"]) {
                let s = uname.trim().to_lowercase();
                if s.contains("darwin") {
                    return "macos".to_string();
                }
                if s.contains("linux") {
                    return "linux".to_string();
                }
            }
            match std::env::consts::OS {
                "macos" => "macos",
                "windows" => "windows",
                _ => "linux",
            }
            .to_string()
        })
        .clone()
}

/// The CPU architecture the process is **actually running on**, probed at
/// runtime. On macOS this detects the *native* arch even when the binary is
/// translated by Rosetta 2 (`sysctl hw.optional.arm64`), so we never pull an
/// x86_64 engine onto Apple Silicon. Maps to the artifact arch token
/// (`x86_64`/`aarch64`).
pub fn host_arch() -> String {
    // Memoized for the same reason as host_platform.
    static CACHE: OnceLock<String> = OnceLock::new();
    CACHE
        .get_or_init(|| {
            if host_platform() == "macos" {
                // Rosetta-translated x86_64 processes still report the *native*
                // arm64 via this sysctl, so we get the right engine slice.
                if let Some(out) = run_trusted("sysctl", &["-n", "hw.optional.arm64"]) {
                    if out.trim() == "1" {
                        return "aarch64".to_string();
                    }
                    return "x86_64".to_string();
                }
            }
            if let Some(m) = run_trusted("uname", &["-m"]) {
                return match m.trim() {
                    "x86_64" | "amd64" => "x86_64".to_string(),
                    "aarch64" | "arm64" => "aarch64".to_string(),
                    other => other.to_string(),
                };
            }
            match std::env::consts::ARCH {
                "aarch64" => "aarch64",
                _ => "x86_64",
            }
            .to_string()
        })
        .clone()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuBackend {
    Cpu,
    Cuda,
    /// Metal is only constructed on macOS (behind `#[cfg(target_os =
    /// "macos")]`). The `allow(dead_code)` prevents a false positive on
    /// Linux builds.
    #[allow(dead_code)]
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

    /// Parse a backend name to its enum variant.
    /// Only used from test code. The allow is because dead_code does not
    /// count `#[cfg(test)]` usage as live.
    #[allow(dead_code)]
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

/// Full detection result for the `/detect-gpu` endpoint (P3).
#[derive(Debug, Clone)]
pub struct GpuDetection {
    /// All backends usable on this host (always includes "cpu").
    pub available: Vec<String>,
    /// The recommended backend (the priority winner).
    pub recommended: String,
    pub platform: String,
    pub arch: String,
}

/// Detect ALL available backends + the recommended one. CPU is
/// always available. Used by the `/detect-gpu` endpoint to power
/// the settings-page GPU card.
pub fn detect_all() -> GpuDetection {
    let mut available = vec![GpuBackend::Cpu.as_str().to_string()];

    if is_cuda_available() {
        available.push(GpuBackend::Cuda.as_str().to_string());
    }
    #[cfg(target_os = "macos")]
    {
        if is_metal_available() {
            available.push(GpuBackend::Metal.as_str().to_string());
        }
    }
    if is_rocm_available() {
        available.push(GpuBackend::Rocm.as_str().to_string());
    }

    GpuDetection {
        recommended: detect_gpu_backend().as_str().to_string(),
        available,
        platform: host_platform(),
        arch: host_arch(),
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

/// Parse the `CUDA Version: X.Y` field out of `nvidia-smi` header output.
/// That field reports the **maximum** CUDA runtime the installed driver
/// supports (driver-version-derived, not toolkit), which is exactly what
/// we match build artifacts against.
fn parse_cuda_smi_version(stdout: &str) -> Option<(u32, u32)> {
    let idx = stdout.find("CUDA Version:")?;
    let rest = &stdout[idx + "CUDA Version:".len()..];
    let tok = rest.split_whitespace().next()?; // e.g. "12.4"
    let mut parts = tok.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next().unwrap_or("0").parse().unwrap_or(0);
    Some((major, minor))
}

/// Parse a ROCm release string like `6.1.2-...` (the contents of
/// `/opt/rocm/.info/version`) into `(major, minor)`.
fn parse_rocm_version_str(s: &str) -> Option<(u32, u32)> {
    let tok = s.trim().split(['-', ' ']).next()?; // "6.1.2"
    let mut parts = tok.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next().unwrap_or("0").parse().unwrap_or(0);
    Some((major, minor))
}

/// Host CUDA version the driver supports (from `nvidia-smi`), if NVIDIA.
fn detect_cuda_version() -> Option<(u32, u32)> {
    let output = probe_trusted("nvidia-smi", &[])?;
    if !output.status.success() {
        return None;
    }
    parse_cuda_smi_version(&String::from_utf8_lossy(&output.stdout))
}

/// Host ROCm release version (from `/opt/rocm/.info/version`), if AMD.
fn detect_rocm_version() -> Option<(u32, u32)> {
    let raw = std::fs::read_to_string("/opt/rocm/.info/version").ok()?;
    parse_rocm_version_str(&raw)
}

/// Extract `(major, minor)` from a backend artifact tag with the given
/// family prefix, e.g. `cuda12.6` + `"cuda"` → `(12, 6)`.
fn parse_backend_version(tag: &str, family: &str) -> Option<(u32, u32)> {
    let rest = tag.strip_prefix(family)?;
    let mut parts = rest.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next().unwrap_or("0").parse().unwrap_or(0);
    Some((major, minor))
}

/// Choose the most suitable backend artifact for a host from the list a
/// release actually published (`available`), given the host's detected GPU
/// versions. Pure (host facts passed in) so it is unit-testable without a
/// real GPU.
///
/// Policy (matches "suitable **major** version"):
/// - macOS → `metal` if published (Apple GPUs are forward/back compatible
///   within the Metal family), else `cpu`.
/// - NVIDIA → among `cuda{maj}.{min}` artifacts with `maj <= host major`,
///   pick the highest (newer drivers run older CUDA toolkits — CUDA is
///   backward compatible; a 12.x build won't run on a driver capped below
///   its major, so we never pick a major above the host).
/// - AMD → among `rocm{maj}.{min}` with `maj == host major` (ROCm has no
///   broad cross-major guarantee), pick the highest minor.
/// - Otherwise → `cpu` if published, else `None`.
pub fn recommend_backend_for(
    os: &str,
    cuda: Option<(u32, u32)>,
    rocm: Option<(u32, u32)>,
    metal: bool,
    available: &[String],
) -> Option<String> {
    let has = |b: &str| available.iter().any(|a| a == b);
    let cpu = || has("cpu").then(|| "cpu".to_string());

    if os == "macos" {
        if metal && has("metal") {
            return Some("metal".to_string());
        }
        return cpu();
    }

    if let Some((host_major, _)) = cuda {
        let best = available
            .iter()
            .filter_map(|tag| parse_backend_version(tag, "cuda").map(|v| (v, tag)))
            .filter(|((maj, _), _)| *maj <= host_major)
            .max_by_key(|((maj, min), _)| (*maj, *min));
        if let Some((_, tag)) = best {
            return Some(tag.clone());
        }
    }

    if let Some((host_major, _)) = rocm {
        let best = available
            .iter()
            .filter_map(|tag| parse_backend_version(tag, "rocm").map(|v| (v, tag)))
            .filter(|((maj, _), _)| *maj == host_major)
            .max_by_key(|((_, min), _)| *min);
        if let Some((_, tag)) = best {
            return Some(tag.clone());
        }
    }

    cpu()
}

/// Host-aware wrapper over [`recommend_backend_for`]: detects this machine's
/// GPU versions and picks the best artifact from `available`.
pub fn recommend_backend(available: &[String]) -> Option<String> {
    let os = host_platform();
    let cuda = if is_cuda_available() { detect_cuda_version() } else { None };
    let rocm = if is_rocm_available() { detect_rocm_version() } else { None };
    let metal = os == "macos" && is_metal_available();
    recommend_backend_for(&os, cuda, rocm, metal, available)
}

fn is_cuda_available() -> bool {
    // Memoized: GPU presence is stable per-process; avoids re-spawning
    // nvidia-smi on every detect-gpu / recommend-backend call (the repeated
    // spawns slowed /detect-gpu enough to 502 on a cold backend).
    static CACHE: OnceLock<bool> = OnceLock::new();
    *CACHE.get_or_init(is_cuda_available_uncached)
}

fn is_cuda_available_uncached() -> bool {
    // Fast path: check for CUDA library files first (instant, no subprocess).
    // The subprocess probe (nvidia-smi) is slower and blocks the async runtime
    // on the init path, so it only runs as a fallback when no instant file check
    // matches.
    #[cfg(target_os = "linux")]
    {
        if std::path::Path::new("/usr/local/cuda/lib64/libcudart.so").exists()
            || std::path::Path::new("/usr/lib/x86_64-linux-gnu/libcudart.so").exists()
        {
            tracing::debug!("Found CUDA libraries in system");
            return true;
        }
    }

    // Fallback: try nvidia-smi command (absolute-path resolved, no PATH lookup).
    // Closes 08-llm-local-runtime F-14 (Low). If the binary is not in
    // any trusted dir we skip this probe.
    if let Some(output) = probe_trusted("nvidia-smi", &[])
        && output.status.success() {
            tracing::debug!("nvidia-smi command succeeded");
            return true;
        }

    false
}

fn is_metal_available() -> bool {
    static CACHE: OnceLock<bool> = OnceLock::new();
    *CACHE.get_or_init(is_metal_available_uncached)
}

fn is_metal_available_uncached() -> bool {
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
            if let Some(output) = probe_trusted("system_profiler", &["SPDisplaysDataType"]) {
                if output.status.success() {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    // Metal is supported on macOS 10.11+ with compatible GPUs
                    if stdout.contains("Metal") {
                        tracing::debug!("Metal support detected via system_profiler");
                        return true;
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
    static CACHE: OnceLock<bool> = OnceLock::new();
    *CACHE.get_or_init(is_rocm_available_uncached)
}

fn is_rocm_available_uncached() -> bool {
    // Fast path: check for ROCm library files first (instant, no subprocess).
    // The subprocess probe (rocm-smi) is slower and blocks the async runtime
    // on the init path, so it only runs as a fallback when no instant file check
    // matches.
    #[cfg(target_os = "linux")]
    {
        if std::path::Path::new("/opt/rocm/lib/libamdhip64.so").exists()
            || std::path::Path::new("/opt/rocm/hip/lib/libamdhip64.so").exists()
        {
            tracing::debug!("Found ROCm libraries in system");
            return true;
        }
    }

    // Fallback: try rocm-smi command (absolute-path resolved, no PATH lookup)
    if let Some(output) = probe_trusted("rocm-smi", &[])
        && output.status.success() {
            tracing::debug!("rocm-smi command succeeded");
            return true;
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

    #[test]
    fn test_parse_cuda_smi_version() {
        let smi = "| NVIDIA-SMI 550.90  Driver Version: 550.90  CUDA Version: 12.4 |";
        assert_eq!(parse_cuda_smi_version(smi), Some((12, 4)));
        assert_eq!(parse_cuda_smi_version("no cuda here"), None);
    }

    #[test]
    fn test_parse_rocm_version_str() {
        assert_eq!(parse_rocm_version_str("6.1.2-12345"), Some((6, 1)));
        assert_eq!(parse_rocm_version_str("5.7\n"), Some((5, 7)));
        assert_eq!(parse_rocm_version_str(""), None);
    }

    #[test]
    fn test_parse_backend_version() {
        assert_eq!(parse_backend_version("cuda12.6", "cuda"), Some((12, 6)));
        assert_eq!(parse_backend_version("rocm6.1", "rocm"), Some((6, 1)));
        assert_eq!(parse_backend_version("cpu", "cuda"), None);
    }

    // The published Linux x86_64 backend set from the release matrix.
    const LINUX: &[&str] = &["cpu", "cuda12.6", "cuda13.0", "rocm5.7", "rocm6.1"];
    fn linux() -> Vec<String> {
        LINUX.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn cuda_picks_highest_minor_within_host_major() {
        // Host driver caps at CUDA 12.x → never 13.0, pick the highest 12.
        let r = recommend_backend_for("linux", Some((12, 4)), None, false, &linux());
        assert_eq!(r.as_deref(), Some("cuda12.6"));
    }

    #[test]
    fn cuda_newer_host_runs_older_toolkit() {
        // Host CUDA 13 with both 12.6 and 13.0 → newest installable major.
        let r = recommend_backend_for("linux", Some((13, 1)), None, false, &linux());
        assert_eq!(r.as_deref(), Some("cuda13.0"));
        // Host CUDA 13 but only 12.x published → 12.x still runs (back-compat).
        let only12 = vec!["cpu".into(), "cuda12.6".into()];
        let r = recommend_backend_for("linux", Some((13, 1)), None, false, &only12);
        assert_eq!(r.as_deref(), Some("cuda12.6"));
    }

    #[test]
    fn rocm_matches_host_major_exactly() {
        let r = recommend_backend_for("linux", None, Some((6, 0)), false, &linux());
        assert_eq!(r.as_deref(), Some("rocm6.1"));
        let r = recommend_backend_for("linux", None, Some((5, 5)), false, &linux());
        assert_eq!(r.as_deref(), Some("rocm5.7"));
        // No artifact for host's ROCm major → fall back to cpu.
        let r = recommend_backend_for("linux", None, Some((4, 0)), false, &linux());
        assert_eq!(r.as_deref(), Some("cpu"));
    }

    #[test]
    fn macos_prefers_metal() {
        let mac = vec!["cpu".into(), "metal".into()];
        let r = recommend_backend_for("macos", None, None, true, &mac);
        assert_eq!(r.as_deref(), Some("metal"));
    }

    #[test]
    fn no_gpu_falls_back_to_cpu() {
        let r = recommend_backend_for("linux", None, None, false, &linux());
        assert_eq!(r.as_deref(), Some("cpu"));
    }

    #[test]
    fn none_when_nothing_published() {
        assert_eq!(recommend_backend_for("linux", Some((12, 4)), None, false, &[]), None);
    }

    #[test]
    fn probe_times_out_instead_of_hanging() {
        // A binary that sleeps far longer than the timeout must return None
        // promptly, not block — this is the guard that keeps a slow cold
        // `nvidia-smi` from stalling `/detect-gpu`.
        let Some(sleep) = resolve_system_binary("sleep") else {
            return; // no /usr/bin/sleep on this host; skip
        };
        let start = std::time::Instant::now();
        let out = probe_command_with_timeout(sleep, &["10"], Duration::from_millis(150));
        assert!(out.is_none(), "a probe exceeding the timeout must yield None");
        assert!(
            start.elapsed() < Duration::from_secs(3),
            "must abandon the wait, not block for the child's full runtime"
        );
    }

    #[test]
    fn probe_returns_output_for_fast_binary() {
        let Some(bin) = resolve_system_binary("uname").or_else(|| resolve_system_binary("true"))
        else {
            return;
        };
        let out = probe_command_with_timeout(bin, &[], Duration::from_secs(3));
        assert!(out.is_some(), "a fast probe should return its output");
    }
}
