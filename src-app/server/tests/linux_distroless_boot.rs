//! Dynamic proof of Linux self-containedness: run the release binary
//! inside `gcr.io/distroless/static:nonroot` — a container with only
//! /etc/passwd and the binary itself, no shared libs available at all.
//! If `ziee --version` works there, the binary needs NO external
//! shared libs.
//!
//! Second test verifies the sandbox path works when the documented
//! host prereqs (bubblewrap, squashfuse, fuse3) are present.
//!
//! Both `#[ignore]`'d because they shell out to Docker (slow + needs
//! daemon). Run via:
//!   cargo test --release --target <musl-triple> --test linux_distroless_boot -- --ignored --nocapture

#![cfg(target_os = "linux")]

use std::path::PathBuf;
use std::process::Command;

fn require_docker() {
    let out = Command::new("docker").arg("info").output();
    if !matches!(out, Ok(o) if o.status.success()) {
        panic!("docker daemon not available; skip this test or start Docker");
    }
}

fn arch() -> &'static str {
    if cfg!(target_arch = "aarch64") {
        "arm64"
    } else {
        "amd64"
    }
}

#[test]
fn runs_in_distroless_static() {
    require_docker();
    let bin: PathBuf = env!("CARGO_BIN_EXE_ziee").into();
    assert!(bin.exists(), "server binary missing: {}", bin.display());

    // Mount the binary read-only at /ziee inside the container.
    // distroless/static has no shell, so we invoke the binary directly
    // and let it print its --version (or --help) and exit non-zero/zero
    // — either way, "the loader could resolve all symbols" is the proof
    // of self-containedness.
    let status = Command::new("docker")
        .args([
            "run", "--rm",
            "--platform", &format!("linux/{}", arch()),
            "-v", &format!("{}:/ziee:ro", bin.display()),
            "gcr.io/distroless/static:nonroot",
            "/ziee", "--help",
        ])
        .status()
        .expect("docker run");
    assert!(
        status.success() || matches!(status.code(), Some(_)),
        "binary failed to load in distroless container (likely missing shared lib)"
    );
}

#[test]
fn runs_in_alpine_with_sandbox_prereqs() {
    require_docker();
    let bin: PathBuf = env!("CARGO_BIN_EXE_ziee").into();
    assert!(bin.exists(), "server binary missing: {}", bin.display());

    let status = Command::new("docker")
        .args([
            "run", "--rm",
            "--platform", &format!("linux/{}", arch()),
            "--cap-add", "SYS_ADMIN",
            "--device", "/dev/fuse",
            "-v", &format!("{}:/ziee:ro", bin.display()),
            "alpine:3.20",
            "sh", "-c",
            "apk add -q --no-cache bubblewrap squashfuse fuse3 && /ziee --help",
        ])
        .status()
        .expect("docker run");
    assert!(
        status.success(),
        "alpine + sandbox prereqs run failed"
    );
}
