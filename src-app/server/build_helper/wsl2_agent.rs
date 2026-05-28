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

    // Cache hit: existing non-empty agent at the destination.
    if agent_dest.exists()
        && fs::metadata(&agent_dest).map(|m| m.len() > 0).unwrap_or(false)
    {
        println!(
            "wsl2-agent: agent already at {} ({} bytes); skipping rebuild",
            agent_dest.display(),
            fs::metadata(&agent_dest)?.len()
        );
        return Ok(());
    }

    println!("wsl2-agent: cross-compiling sandbox-guest-agent for x86_64-unknown-linux-musl via {RUST_MUSL_IMAGE}");
    require_docker()?;
    let built = build_guest_agent(workspace, out_dir)?;
    fs::copy(&built, &agent_dest)?;
    println!(
        "wsl2-agent: wrote {} ({} bytes)",
        agent_dest.display(),
        fs::metadata(&agent_dest)?.len()
    );
    Ok(())
}

fn require_docker() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::var_os("PATH").ok_or("PATH not set")?;
    for dir in std::env::split_paths(&path) {
        // Both `docker` and `docker.exe` are acceptable; Windows
        // resolution searches PATHEXT.
        if dir.join("docker").is_file() || dir.join("docker.exe").is_file() {
            return Ok(());
        }
    }
    Err(
        "wsl2-agent: `docker` not on PATH; install Docker Desktop for Windows so \
         the agent can be cross-compiled into the release binary. (Dev/test \
         workaround: `scripts/build-sandbox-agent-linux.sh` + the runtime's \
         sibling-of-exe lookup, or set `ZIEE_SKIP_WSL2_AGENT_BUNDLE=1`.)"
            .into(),
    )
}

/// Cross-compile the agent to x86_64-unknown-linux-musl (the arch that
/// WSL2 runs on Windows hosts — x86_64 / amd64). Uses the same
/// rust:1.90-alpine3.20 image the macOS path uses, with a per-build
/// CARGO_TARGET_DIR to avoid colliding with the parent server's
/// `target/`.
fn build_guest_agent(
    workspace: &Path,
    out_dir: &str,
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
    let mount_src = format!("type=bind,source={src_app_clean},target=/work");
    let mount_target = format!("type=bind,source={docker_target_clean},target=/cargo-target");

    let status = Command::new("docker")
        .args([
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
        ])
        .status()?;
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
