//! Static-analysis proof that the mac-arm64 release binary AND every
//! Mach-O in the embedded sandbox-runtime bundle have zero references
//! to `/opt/homebrew/` or `/usr/local/`. If this test passes, the
//! single `ziee` binary truly needs no brew packages at runtime.
//!
//! Lives alongside `integration_tests.rs` etc.; run via:
//!   cargo test --release --target aarch64-apple-darwin --test macos_self_contained
//!
//! Skipped on every other target (the bundle is empty there).

#![cfg(all(target_os = "macos", target_arch = "aarch64"))]

use std::path::{Path, PathBuf};
use std::process::Command;

/// `otool -L` output paths that ARE allowed in a self-contained bundle.
/// Anything else (notably anything starting with `/opt/homebrew/` or
/// `/usr/local/`) is a bug — that's a runtime dep we forgot to bundle
/// or rewrite.
fn is_allowed_dep(path: &str) -> bool {
    path.starts_with("/System/")
        || path.starts_with("/usr/lib/")
        || path.starts_with("@rpath/")
        || path.starts_with("@executable_path/")
        || path.starts_with("@loader_path/")
}

fn collect_macho_deps(file: &Path) -> Vec<String> {
    let out = Command::new("otool")
        .args(["-L", &file.to_string_lossy()])
        .output()
        .expect("otool exec");
    if !out.status.success() {
        // otool exits non-zero on non-Mach-O files. Skip silently.
        return Vec::new();
    }
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .skip(1) // first line is the file path itself
        .filter_map(|l| l.trim().split_whitespace().next().map(String::from))
        .collect()
}

fn assert_no_brew_refs(file: &Path) {
    let deps = collect_macho_deps(file);
    let bad: Vec<&String> = deps.iter().filter(|d| !is_allowed_dep(d)).collect();
    assert!(
        bad.is_empty(),
        "{} references non-system paths: {:#?}",
        file.display(),
        bad
    );
}

#[test]
fn server_binary_has_no_brew_refs() {
    // CARGO_BIN_EXE_* is set automatically by cargo for integration tests.
    let server: PathBuf = env!("CARGO_BIN_EXE_ziee").into();
    assert!(server.exists(), "server binary missing: {}", server.display());
    assert_no_brew_refs(&server);
}

#[test]
fn embedded_bundle_machos_have_no_brew_refs() {
    use ziee::code_sandbox_embedded as embedded;
    assert!(
        embedded::is_supported(),
        "bundle is empty — either built for an unsupported target or with \
         ZIEE_SKIP_SANDBOX_BUNDLE=1. Rebuild without the skip flag."
    );

    let extracted = embedded::ensure().expect("extract bundle");
    // Flat-layout: launcher at <app_data>/bin/, dylibs at <app_data>/lib/.
    // Scope the Mach-O walk to JUST our bundle's owned paths so we don't
    // accidentally scrutinize sibling extracted artifacts (pandoc,
    // libpdfium, uv, bun) that share <app_data>/bin/ — those have their
    // own self-containment story, owned by file/utils/embedded.rs.
    let app_data = extracted
        .launcher
        .parent()
        .and_then(|p| p.parent())
        .expect("app_data root");
    let lib_dir = app_data.join("lib");

    let mut machos = vec![extracted.launcher.clone()];
    for entry in std::fs::read_dir(&lib_dir).expect("read lib/").flatten() {
        let p = entry.path();
        if is_macho(&p) {
            machos.push(p);
        }
    }
    assert!(machos.len() >= 2, "expected ≥2 Mach-Os in bundle (launcher + dylibs), got {machos:?}");
    for path in &machos {
        assert_no_brew_refs(path);
    }
}

#[test]
fn launcher_has_hypervisor_entitlement() {
    use ziee::code_sandbox_embedded as embedded;
    let extracted = embedded::ensure().expect("extract bundle");
    let out = Command::new("codesign")
        .args(["-d", "--entitlements", "-", &extracted.launcher.to_string_lossy()])
        .output()
        .expect("codesign exec");
    assert!(out.status.success(), "codesign -d failed: {}", String::from_utf8_lossy(&out.stderr));
    // Newer codesign writes the plist to stderr.
    let dump = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        dump.contains("com.apple.security.hypervisor"),
        "launcher missing hypervisor entitlement. Got: {dump}"
    );
}

fn walk_machos(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    walk(root, &mut out);
    out
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for entry in rd.flatten() {
        let p = entry.path();
        if p.is_dir() {
            walk(&p, out);
        } else if is_macho(&p) {
            out.push(p);
        }
    }
}

fn is_macho(p: &Path) -> bool {
    let mut header = [0u8; 4];
    use std::io::Read;
    let Ok(mut f) = std::fs::File::open(p) else { return false };
    if f.read_exact(&mut header).is_err() {
        return false;
    }
    // Mach-O 64-bit magic numbers (LE: feedfacf, BE: cffaedfe).
    matches!(
        u32::from_be_bytes(header),
        0xfeedfacf | 0xcffaedfe | 0xfeedface | 0xcefaedfe
    )
}
