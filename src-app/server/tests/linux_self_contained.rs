//! Static-analysis proof that the Linux release binary is fully
//! self-contained: the only dynamic deps are the musl loader + libc
//! (or, ideally, none at all because the binary is fully statically
//! linked). Also asserts `cargo tree` shows no `native-tls` in the
//! workspace and that `openssl-sys` only survives via git2-vendored.
//!
//! Run via:
//!   cargo test --release --target aarch64-unknown-linux-musl --test linux_self_contained
//!   cargo test --release --target x86_64-unknown-linux-musl  --test linux_self_contained
//!
//! Skipped on non-Linux targets.

#![cfg(target_os = "linux")]

use std::path::PathBuf;
use std::process::Command;

#[test]
fn binary_is_statically_linked_or_musl_only() {
    let bin: PathBuf = env!("CARGO_BIN_EXE_ziee").into();
    assert!(bin.exists(), "server binary missing: {}", bin.display());

    let file_out = Command::new("file")
        .arg(&bin)
        .output()
        .expect("file exec");
    let file_str = String::from_utf8_lossy(&file_out.stdout);

    // The musl static build reports either "statically linked" OR
    // "dynamically linked, interpreter /lib/ld-musl-*" depending on
    // how the link was done. Both are acceptable as long as ldd only
    // shows musl loader / libc / vdso.
    if file_str.contains("statically linked") {
        return; // ideal case
    }

    let ldd = Command::new("ldd").arg(&bin).output().expect("ldd exec");
    let ldd_str = String::from_utf8_lossy(&ldd.stdout);
    for line in ldd_str.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let allowed = line.contains("linux-vdso")
            || line.contains("/lib/ld-musl-")
            || line.contains("libc.musl-")
            || line.contains("statically linked");
        assert!(
            allowed,
            "unexpected dynamic dep: {line:?}\nfull ldd:\n{ldd_str}"
        );
    }
}

#[test]
fn dep_graph_has_no_native_tls() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let out = Command::new(env!("CARGO"))
        .args(["tree", "-i", "native-tls"])
        .current_dir(&manifest)
        .output()
        .expect("cargo tree exec");
    // `cargo tree -i <pkg>` exits with status 101 + "did not match any packages"
    // when the package isn't in the graph — that's what we want.
    let stderr = String::from_utf8_lossy(&out.stderr);
    if out.status.success() {
        let stdout = String::from_utf8_lossy(&out.stdout);
        panic!("native-tls still in dep graph:\n{stdout}");
    }
    assert!(
        stderr.contains("did not match any packages"),
        "unexpected cargo-tree failure: {stderr}"
    );
}

#[test]
fn openssl_sys_only_via_git2_vendored() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let out = Command::new(env!("CARGO"))
        .args(["tree", "-i", "openssl-sys"])
        .current_dir(&manifest)
        .output()
        .expect("cargo tree exec");
    let stdout = String::from_utf8_lossy(&out.stdout);
    // Must be at most one chain, going through libgit2-sys → git2.
    // The vendored-openssl feature pulls openssl-src which statically
    // links openssl into the binary (no runtime dylib dep).
    assert!(
        stdout.contains("libgit2-sys"),
        "openssl-sys chain unexpected:\n{stdout}"
    );
    assert!(
        !stdout.contains("reqwest"),
        "reqwest is still pulling openssl-sys somehow:\n{stdout}"
    );
    assert!(
        !stdout.contains("ldap3"),
        "ldap3 is still pulling openssl-sys:\n{stdout}"
    );
    assert!(
        !stdout.contains("sigstore"),
        "sigstore is still pulling openssl-sys:\n{stdout}"
    );
}
