//! Tier 3 — updater signing round-trip (self-contained, no secrets).
//!
//! Proves the full production sign → manifest → verify contract:
//!   1. Generate an ephemeral updater keypair with `tauri signer generate`
//!      (the exact tool tauri-action uses in CI).
//!   2. Sign a dummy "update" artifact with `tauri signer sign`.
//!   3. Assemble `latest.json` with the shared production script
//!      (`scripts/updater/build-latest-json.mjs`).
//!   4. Verify the base64 signature embedded in `latest.json` against the
//!      generated public key using `minisign-verify` — the SAME crate the
//!      runtime `tauri-plugin-updater` uses to verify downloads
//!      (see its `verify_signature`). A tampered artifact must FAIL.
//!
//! No GitHub release, no committed key, no network. Needs Node + the
//! workspace `@tauri-apps/cli` (a devDependency) on PATH, which `npx`
//! resolves from the repo-root node_modules.

use std::path::{Path, PathBuf};
use std::process::Command;

use base64::Engine;
use minisign_verify::{PublicKey, Signature};

fn repo_root() -> PathBuf {
    // CARGO_MANIFEST_DIR = <repo>/src-app/desktop/tauri
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .canonicalize()
        .expect("canonicalize repo root")
}

fn run(label: &str, cmd: &mut Command) {
    let out = cmd.output().unwrap_or_else(|e| panic!("{label}: spawn failed: {e}"));
    if !out.status.success() {
        panic!(
            "{label} failed ({})\n--- stdout ---\n{}\n--- stderr ---\n{}",
            out.status,
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr),
        );
    }
}

/// Replicates `tauri-plugin-updater`'s `verify_signature` exactly:
/// base64-decode both the tauri pubkey and the manifest signature into
/// minisign file text, decode them, and verify.
fn verify(tauri_pubkey: &str, manifest_signature: &str, data: &[u8]) -> Result<(), String> {
    let pub_decoded = base64::engine::general_purpose::STANDARD
        .decode(tauri_pubkey.trim())
        .map_err(|e| format!("pubkey b64: {e}"))?;
    let pub_str = std::str::from_utf8(&pub_decoded).map_err(|e| format!("pubkey utf8: {e}"))?;
    let public_key = PublicKey::decode(pub_str).map_err(|e| format!("PublicKey::decode: {e}"))?;

    let sig_decoded = base64::engine::general_purpose::STANDARD
        .decode(manifest_signature.trim())
        .map_err(|e| format!("sig b64: {e}"))?;
    let sig_str = std::str::from_utf8(&sig_decoded).map_err(|e| format!("sig utf8: {e}"))?;
    let signature = Signature::decode(sig_str).map_err(|e| format!("Signature::decode: {e}"))?;

    public_key
        .verify(data, &signature, true)
        .map_err(|e| format!("verify: {e}"))
}

#[test]
fn updater_signature_round_trip_verifies_and_rejects_tampering() {
    let root = repo_root();
    let tmp = tempfile::tempdir().expect("tempdir");
    let key_path = tmp.path().join("updater.key");
    let pub_path = tmp.path().join("updater.key.pub");

    // 1. Generate an ephemeral keypair (empty password, non-interactive).
    run(
        "tauri signer generate",
        Command::new("npx")
            .current_dir(&root)
            .args(["--no-install", "tauri", "signer", "generate", "--ci", "-f", "-p", ""])
            .arg("-w")
            .arg(&key_path),
    );
    assert!(pub_path.exists(), "public key not produced");

    // 2. A dummy "update" artifact, named with the Tauri platform key so the
    //    manifest builder maps it to `darwin-aarch64`.
    let artifacts_dir = tmp.path().join("artifacts");
    std::fs::create_dir_all(&artifacts_dir).unwrap();
    let artifact = artifacts_dir.join("ziee_9.9.9_darwin-aarch64.app.tar.gz");
    let payload = b"pretend this is a compressed app bundle \x00\x01\x02 the update payload";
    std::fs::write(&artifact, payload).unwrap();

    // 3. Sign it -> writes <artifact>.sig (base64 of the minisign signature).
    run(
        "tauri signer sign",
        Command::new("npx")
            .current_dir(&root)
            .args(["--no-install", "tauri", "signer", "sign", "-p", ""])
            .arg("-f")
            .arg(&key_path)
            .arg(&artifact),
    );
    assert!(
        artifact.with_extension("tar.gz.sig").exists()
            || artifacts_dir.join("ziee_9.9.9_darwin-aarch64.app.tar.gz.sig").exists(),
        "signature file not produced"
    );

    // 4. Assemble latest.json with the SHARED production script.
    let latest = tmp.path().join("latest.json");
    run(
        "build-latest-json.mjs",
        Command::new("node")
            .current_dir(&root)
            .arg("scripts/updater/build-latest-json.mjs")
            .arg("--artifacts-dir")
            .arg(&artifacts_dir)
            .args([
                "--base-url",
                "https://github.com/ziee-ai/ziee/releases/download/v9.9.9",
                "--version",
                "9.9.9",
            ])
            .arg("--out")
            .arg(&latest),
    );

    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&latest).unwrap()).expect("parse latest.json");
    let entry = &manifest["platforms"]["darwin-aarch64"];
    let signature = entry["signature"].as_str().expect("signature string");
    let url = entry["url"].as_str().expect("url string");
    assert!(url.ends_with("ziee_9.9.9_darwin-aarch64.app.tar.gz"), "url = {url}");

    let pubkey = std::fs::read_to_string(&pub_path).unwrap();

    // 5a. The genuine artifact verifies against the manifest signature.
    verify(&pubkey, signature, payload)
        .expect("genuine artifact must verify against the manifest signature");

    // 5b. A tampered artifact must FAIL verification.
    let mut tampered = payload.to_vec();
    tampered[0] ^= 0xFF;
    assert!(
        verify(&pubkey, signature, &tampered).is_err(),
        "tampered artifact must NOT verify"
    );

    // 5c. A bogus signature must also fail (guards against accidental
    //     "always Ok" verifier wiring).
    let bogus = base64::engine::general_purpose::STANDARD.encode(
        "untrusted comment: x\nRWXX\ntrusted comment: y\nAAAA\n",
    );
    assert!(
        verify(&pubkey, &bogus, payload).is_err(),
        "bogus signature must NOT verify"
    );
}
