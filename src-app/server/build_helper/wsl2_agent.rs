//! Build-time cross-compile of `sandbox-guest-agent` for WSL2 (Windows host).
//!
//! Mirrors the macOS path (`build_helper/sandbox_runtime.rs`) but minimal:
//! the Windows backend only needs the Linux agent ELF, not a launcher +
//! dylibs + Alpine guest-root bundle. So we drop a single static-musl
//! binary at
//! `binaries/<host-target>/sandbox-runtime/ziee-sandbox-agent` and let
//! `code_sandbox::wsl2_agent_embedded` `include_bytes!` it.
//!
//! On every non-Windows target this is a no-op — we still write a 0-byte
//! placeholder so the `include_bytes!` macro resolves in any host build
//! (the runtime layer checks the embedded blob is non-empty before using
//! it).

#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const RUST_MUSL_IMAGE: &str = "rust:1.90-alpine3.20";

/// Entry point called from `build.rs`. Writes the agent binary to
/// `<binaries>/<target>/sandbox-runtime/ziee-sandbox-agent`. Returns
/// `Ok(())` on every target; non-Windows targets get a placeholder
/// 0-byte file so the runtime `include_bytes!` always resolves.
pub fn setup(
    target: &str,
    target_dir: &Path,
    out_dir: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let bundle_dir = target_dir.join("sandbox-runtime");
    fs::create_dir_all(&bundle_dir)?;
    let agent_dest = bundle_dir.join("ziee-sandbox-agent");

    // Guarantee the `include_bytes!` target ALWAYS exists, on every target,
    // before any fallible step below. `wsl2_agent_embedded` unconditionally
    // `include_bytes!`s this path on Windows; if the cross-compile fails (no
    // Docker on the Windows host PATH — it lives inside WSL — no network,
    // etc.) and we returned `Err` without a file here, the crate would fail
    // to COMPILE ("couldn't read ... ziee-sandbox-agent: os error 2") rather
    // than fail soft. A successful Windows cross-compile overwrites this
    // 0-byte placeholder with the real ELF; otherwise it stands and the
    // runtime `is_supported()` gate returns false (sibling-of-exe fallback).
    if !agent_dest.exists() {
        fs::write(&agent_dest, b"")?;
    }

    if !target.contains("windows") {
        println!(
            "wsl2-agent: skipping (target = {target}, not *-windows-*)"
        );
        if !agent_dest.exists() {
            fs::write(&agent_dest, b"")?;
        }
        return Ok(());
    }

    // Escape hatch for fast iteration on unrelated code. Without the real
    // agent embedded, the WSL2 backend's `agent_host_path()` falls back
    // to the sibling-of-exe lookup (the `scripts/build-sandbox-agent-linux.sh`
    // output). Useful in dev, never appropriate for a release build.
    if std::env::var_os("ZIEE_SKIP_WSL2_AGENT_BUNDLE").is_some() {
        println!(
            "wsl2-agent: ZIEE_SKIP_WSL2_AGENT_BUNDLE set; writing placeholder"
        );
        if !agent_dest.exists() {
            fs::write(&agent_dest, b"")?;
        }
        return Ok(());
    }

    // Rerun-if-changed: every source file that influences the agent.
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").map(PathBuf::from)?;
    let workspace = manifest_dir
        .parent()
        .ok_or("no workspace parent for CARGO_MANIFEST_DIR")?;
    for src in &[
        "sandbox-guest-agent/src/main.rs",
        "sandbox-guest-agent/Cargo.toml",
        "sandbox-seccomp/src/lib.rs",
        "sandbox-vm-protocol/src/lib.rs",
    ] {
        println!("cargo:rerun-if-changed={}", workspace.join(src).display());
    }

    // Cache hit ONLY when the embedded agent is at least as new as every one
    // of its sources. The `cargo:rerun-if-changed` lines above re-run this
    // build script when the agent source changes, but a plain
    // "non-empty file exists" check would still early-return the STALE binary
    // — freezing the embedded agent at whatever version first populated
    // `agent_dest`. That bug shipped a pre-long-lived agent (no `StartProcess`
    // / tag 6 support) into builds whose source already had it, so every
    // long-lived/MCP sandbox call failed with "unknown frame tag 6". Compare
    // mtimes and rebuild when any source is newer.
    let agent_fresh = agent_dest.exists()
        && fs::metadata(&agent_dest).map(|m| m.len() > 0).unwrap_or(false)
        && {
            let dest_mtime = fs::metadata(&agent_dest).and_then(|m| m.modified()).ok();
            let newest_src = [
                "sandbox-guest-agent/src/main.rs",
                "sandbox-guest-agent/Cargo.toml",
                "sandbox-seccomp/src/lib.rs",
                "sandbox-vm-protocol/src/lib.rs",
            ]
            .iter()
            .filter_map(|s| {
                fs::metadata(workspace.join(s)).and_then(|m| m.modified()).ok()
            })
            .max();
            match (dest_mtime, newest_src) {
                // Embedded agent is newer than (or equal to) every source.
                (Some(dest), Some(src)) => dest >= src,
                // Can't determine mtimes → rebuild to be safe.
                _ => false,
            }
        };
    if agent_fresh {
        println!(
            "wsl2-agent: agent at {} ({} bytes) is newer than its sources; skipping rebuild",
            agent_dest.display(),
            fs::metadata(&agent_dest)?.len()
        );
        return Ok(());
    }
    if agent_dest.exists() {
        println!(
            "wsl2-agent: embedded agent is stale relative to sources; \
             rebuilding (this is the fix for the 'unknown frame tag 6' bug)"
        );
    }

    let driver = detect_docker().ok_or(
        "wsl2-agent: no Docker available to cross-compile the agent. Looked for \
         `docker`/`docker.exe` on PATH (Docker Desktop) and for Docker inside the \
         default WSL distro (`wsl -- docker`). Install one, or set \
         `ZIEE_SKIP_WSL2_AGENT_BUNDLE=1` for a placeholder build (the runtime then \
         falls back to the sibling-of-exe agent lookup).",
    )?;
    println!(
        "wsl2-agent: cross-compiling sandbox-guest-agent for x86_64-unknown-linux-musl \
         via {driver:?} docker / {RUST_MUSL_IMAGE}"
    );
    let built = build_guest_agent(workspace, out_dir, driver)?;
    fs::copy(&built, &agent_dest)?;
    println!(
        "wsl2-agent: wrote {} ({} bytes)",
        agent_dest.display(),
        fs::metadata(&agent_dest)?.len()
    );
    Ok(())
}

/// Which Docker we drive to cross-compile the agent.
#[derive(Clone, Copy, Debug)]
enum DockerDriver {
    /// `docker`/`docker.exe` on PATH (Docker Desktop, or a native CLI).
    /// `--mount` source paths are passed as host (Windows) paths — the
    /// Docker Desktop VM translates them.
    Native,
    /// Docker running *inside* the default WSL distro, reached via
    /// `wsl -- docker ...`. Common on Windows build hosts that run
    /// docker in WSL rather than installing Docker Desktop. `--mount`
    /// source paths must be WSL paths (`/mnt/c/...`).
    Wsl,
}

/// Find a Docker we can use: prefer one on PATH (Docker Desktop), then
/// fall back to Docker inside the default WSL distro. Returns `None` when
/// neither is available (caller fails soft to the placeholder).
fn detect_docker() -> Option<DockerDriver> {
    if let Some(path) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path) {
            // Both `docker` and `docker.exe` are acceptable; Windows
            // resolution searches PATHEXT.
            if dir.join("docker").is_file() || dir.join("docker.exe").is_file() {
                return Some(DockerDriver::Native);
            }
        }
    }
    // Windows host with no Docker on PATH: many build hosts run Docker
    // inside WSL (the same place this repo runs its pgvector build DB)
    // instead of Docker Desktop. Probe `wsl -- docker version`.
    if cfg!(windows) {
        let reachable = Command::new("wsl")
            .args(["--", "docker", "version"])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if reachable {
            return Some(DockerDriver::Wsl);
        }
    }
    None
}

/// Translate a Windows path (`C:\Users\x\y`) to its WSL mount equivalent
/// (`/mnt/c/Users/x/y`) so a Docker daemon running inside WSL can bind it.
fn win_to_wsl_mount(p: &str) -> String {
    let p = p.strip_prefix(r"\\?\").unwrap_or(p);
    if let Some((drive, rest)) = p.split_once(':') {
        if drive.len() == 1 && drive.chars().all(|c| c.is_ascii_alphabetic()) {
            let rest = rest.replace('\\', "/");
            return format!("/mnt/{}/{}", drive.to_ascii_lowercase(), rest.trim_start_matches('/'));
        }
    }
    p.replace('\\', "/")
}

/// Cross-compile the agent to x86_64-unknown-linux-musl (the arch that
/// WSL2 runs on Windows hosts — x86_64 / amd64). Uses the same
/// rust:1.90-alpine3.20 image the macOS path uses, with a per-build
/// CARGO_TARGET_DIR to avoid colliding with the parent server's
/// `target/`.
fn build_guest_agent(
    workspace: &Path,
    out_dir: &str,
    driver: DockerDriver,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let docker_target = PathBuf::from(out_dir).join("wsl2-guest-agent-target");
    fs::create_dir_all(&docker_target)?;

    // Docker on Windows: `-v <src>:<dst>` splits on `:` and the `C:` drive
    // letter's colon breaks the parser ("invalid mode: /work" error).
    // `--mount` uses named comma-separated fields and avoids the colon
    // ambiguity entirely. Strip the `\\?\` long-path prefix that
    // `canonicalize()` adds on Windows — Docker's path validator chokes
    // on it ("source path must be an absolute path").
    let src_app = workspace.canonicalize()?;
    let src_app_str = src_app.to_string_lossy();
    let src_app_clean = src_app_str
        .strip_prefix(r"\\?\")
        .unwrap_or(&src_app_str);
    let docker_target_str = docker_target.to_string_lossy();
    let docker_target_clean = docker_target_str
        .strip_prefix(r"\\?\")
        .unwrap_or(&docker_target_str);
    // For the WSL driver the Docker daemon lives inside WSL, so bind sources
    // must be WSL paths (`/mnt/c/...`); for the Native (Docker Desktop)
    // driver they stay host (Windows) paths. The cargo-target bind points at
    // the SAME on-disk location either way, so reading the built binary back
    // via the Windows `docker_target` path below works for both.
    let (src_bind, tgt_bind) = match driver {
        DockerDriver::Native => (src_app_clean.to_string(), docker_target_clean.to_string()),
        DockerDriver::Wsl => (
            win_to_wsl_mount(src_app_clean),
            win_to_wsl_mount(docker_target_clean),
        ),
    };
    let mount_src = format!("type=bind,source={src_bind},target=/work");
    let mount_target = format!("type=bind,source={tgt_bind},target=/cargo-target");

    let docker_args = [
        "run",
        "--rm",
        "--platform",
        "linux/amd64",
        "--mount",
        &mount_src,
        "--mount",
        &mount_target,
        "-w",
        "/work/sandbox-guest-agent",
        "-e",
        "CARGO_TARGET_DIR=/cargo-target",
        "-e",
        "CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_RUSTFLAGS=-C target-feature=+crt-static",
        "-e",
        "LIBSECCOMP_LINK_TYPE=static",
        "-e",
        "LIBSECCOMP_LIB_PATH=/usr/lib",
        RUST_MUSL_IMAGE,
        "sh",
        "-c",
        "apk add -q libseccomp-dev libseccomp-static musl-dev build-base pkgconf >/dev/null && \
         cargo build --release --target x86_64-unknown-linux-musl 2>&1 | tail -5",
    ];

    // Native: `docker run ...`. Wsl: `wsl -- docker run ...` (default distro).
    let mut cmd = match driver {
        DockerDriver::Native => Command::new("docker"),
        DockerDriver::Wsl => {
            let mut c = Command::new("wsl");
            c.arg("--").arg("docker");
            c
        }
    };
    let status = cmd.args(docker_args).status()?;
    if !status.success() {
        return Err("wsl2-agent: cross-compile via docker failed".into());
    }
    let out = docker_target.join("x86_64-unknown-linux-musl/release/ziee-sandbox-agent");
    if !out.exists() {
        return Err(format!(
            "wsl2-agent: cross-compile reported success but binary missing at {}",
            out.display()
        )
        .into());
    }
    Ok(out)
}
