//! Bridge TLS certificate mint + cache (ITEM-4).
//!
//! The Office task pane is served over `https://localhost:44300` inside Office's
//! WebView2, which refuses an un-trusted cert. We therefore mint an in-process
//! self-signed cert (CN=`localhost`) whose SAN covers `localhost`, `127.0.0.1`,
//! AND `::1` (DEC-5 — WebView2 resolves `localhost` to `::1`), marked
//! `basicConstraints CA:true` so it can serve as its own trust anchor once the
//! `[Connect]` flow installs it into the OS root store (ITEM-13).
//!
//! The signature is ECDSA P-256 / SHA-256 (rcgen's default with a generated key
//! pair). The minted DER + PEM are cached under the app data dir so the *same*
//! trusted cert survives restarts (re-minting would break the installed trust).
//!
//! This module only mints + caches; the rustls listener that serves with it is
//! ITEM-5. The cert bytes are also what `OfficePlatform::install_cert_trust`
//! (ITEM-7) pushes into the OS trust store.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::path::Path;

use axum::http::StatusCode;
use sha2::{Digest, Sha256};

use crate::common::AppError;

/// Cache file names under the app data dir. The `.key.pem` holds the private
/// key; both are re-read on the next boot so the trusted cert persists.
const CERT_FILE: &str = "office-bridge-cert.pem";
const KEY_FILE: &str = "office-bridge-cert.key.pem";

/// A freshly minted (or loaded-from-cache) bridge certificate.
pub struct MintedCert {
    /// DER encoding of the certificate (what `install_cert_trust` pushes into
    /// the OS root store, and what the fingerprint is computed over).
    pub cert_der: Vec<u8>,
    /// PEM encoding of the certificate (fed to rustls + written to the cache).
    pub cert_pem: String,
    /// PEM encoding of the PKCS#8 private key (fed to rustls + written to the
    /// cache). Never leaves the host.
    pub key_pem: String,
    /// Lowercase hex SHA-256 of `cert_der` — a stable identity persisted in
    /// `office_bridge_settings.cert_fingerprint` (DEC-8) and shown in the UI.
    pub fingerprint: String,
}

/// Mint a fresh self-signed `localhost` bridge certificate (DEC-5).
///
/// SAN = DNS `localhost` + IP `127.0.0.1` + IP `::1`; CN = `localhost`;
/// `basicConstraints CA:true`; ECDSA P-256 / SHA-256.
pub fn mint_localhost_cert() -> Result<MintedCert, AppError> {
    use rcgen::{
        BasicConstraints, CertificateParams, DistinguishedName, DnType, IsCa, KeyPair, SanType,
    };

    let mut params = CertificateParams::default();

    let mut dn = DistinguishedName::new();
    dn.push(DnType::CommonName, "localhost");
    params.distinguished_name = dn;

    // SAN: the three names WebView2 may resolve `localhost` to (DEC-5).
    params.subject_alt_names = vec![
        SanType::DnsName(
            "localhost"
                .try_into()
                .map_err(|e| mint_err(format!("bad DNS SAN: {e}")))?,
        ),
        SanType::IpAddress(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1))),
        SanType::IpAddress(IpAddr::V6(Ipv6Addr::LOCALHOST)),
    ];

    // Self-signed trust anchor: mark it a CA so the OS root-store install
    // (ITEM-13) makes the served https://localhost origin trusted.
    params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);

    let key_pair = KeyPair::generate().map_err(|e| mint_err(format!("key generation: {e}")))?;
    let cert = params
        .self_signed(&key_pair)
        .map_err(|e| mint_err(format!("self-sign: {e}")))?;

    let cert_der = cert.der().to_vec();
    let cert_pem = cert.pem();
    let key_pem = key_pair.serialize_pem();
    let fingerprint = fingerprint_hex(&cert_der);

    Ok(MintedCert {
        cert_der,
        cert_pem,
        key_pem,
        fingerprint,
    })
}

/// Return the cached bridge cert under `data_dir` if present + still parseable,
/// else mint a fresh one and write it to the cache (so the trusted cert
/// persists across restarts). The cache is a `<data_dir>/office-bridge-cert.pem`
/// + `.key.pem` pair.
pub fn load_or_mint(data_dir: &Path) -> Result<MintedCert, AppError> {
    let cert_path = data_dir.join(CERT_FILE);
    let key_path = data_dir.join(KEY_FILE);

    if cert_path.exists() && key_path.exists() {
        match load_cached(&cert_path, &key_path) {
            Ok(minted) => return Ok(minted),
            Err(e) => {
                // A corrupt/unparseable cache must not be fatal — re-mint over it.
                tracing::warn!(
                    error = %e,
                    "office_bridge: cached bridge cert unusable; re-minting"
                );
            }
        }
    }

    let minted = mint_localhost_cert()?;
    std::fs::create_dir_all(data_dir)
        .map_err(|e| mint_err(format!("create data dir {}: {e}", data_dir.display())))?;
    std::fs::write(&cert_path, &minted.cert_pem)
        .map_err(|e| mint_err(format!("write {}: {e}", cert_path.display())))?;
    std::fs::write(&key_path, &minted.key_pem)
        .map_err(|e| mint_err(format!("write {}: {e}", key_path.display())))?;
    Ok(minted)
}

/// Read + validate a cached PEM cert/key pair, reconstructing a [`MintedCert`].
/// Errors (missing PEM blocks, an unparseable key, or a cert/key that rustls
/// rejects) bubble up so `load_or_mint` re-mints.
fn load_cached(cert_path: &Path, key_path: &Path) -> Result<MintedCert, AppError> {
    let cert_pem = std::fs::read_to_string(cert_path)
        .map_err(|e| mint_err(format!("read {}: {e}", cert_path.display())))?;
    let key_pem = std::fs::read_to_string(key_path)
        .map_err(|e| mint_err(format!("read {}: {e}", key_path.display())))?;

    // Parse the leaf cert DER out of the PEM (first CERTIFICATE block).
    let cert_der = first_cert_der(&cert_pem)?;

    // Prove the pair actually loads as a rustls server cert before trusting the
    // cache; a pair rustls rejects is treated as unusable (→ re-mint).
    build_server_config(&cert_pem, &key_pem)?;

    let fingerprint = fingerprint_hex(&cert_der);
    Ok(MintedCert {
        cert_der,
        cert_pem,
        key_pem,
        fingerprint,
    })
}

/// Extract the first `CERTIFICATE` DER blob from a PEM string.
fn first_cert_der(cert_pem: &str) -> Result<Vec<u8>, AppError> {
    let mut reader = std::io::BufReader::new(cert_pem.as_bytes());
    let first = rustls_pemfile::certs(&mut reader)
        .next()
        .ok_or_else(|| mint_err("no CERTIFICATE block in cached PEM"))?
        .map_err(|e| mint_err(format!("parse cached cert PEM: {e}")))?;
    Ok(first.as_ref().to_vec())
}

/// Build a single-cert rustls `ServerConfig` from PEM cert + key. Shared by the
/// cache-validation path here and (later) the ITEM-5 listener. Uses the `ring`
/// crypto provider explicitly so it does not depend on a process-wide default
/// provider being installed.
pub fn build_server_config(
    cert_pem: &str,
    key_pem: &str,
) -> Result<rustls::ServerConfig, AppError> {
    use rustls::pki_types::{CertificateDer, PrivateKeyDer};

    let certs: Vec<CertificateDer<'static>> = {
        let mut reader = std::io::BufReader::new(cert_pem.as_bytes());
        rustls_pemfile::certs(&mut reader)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| mint_err(format!("parse cert PEM: {e}")))?
    };
    if certs.is_empty() {
        return Err(mint_err("no certificates in PEM"));
    }

    let key: PrivateKeyDer<'static> = {
        let mut reader = std::io::BufReader::new(key_pem.as_bytes());
        rustls_pemfile::private_key(&mut reader)
            .map_err(|e| mint_err(format!("parse key PEM: {e}")))?
            .ok_or_else(|| mint_err("no private key in PEM"))?
    };

    let provider = std::sync::Arc::new(rustls::crypto::ring::default_provider());
    rustls::ServerConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .map_err(|e| mint_err(format!("rustls protocol versions: {e}")))?
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .map_err(|e| mint_err(format!("rustls single-cert: {e}")))
}

/// Lowercase hex SHA-256 of the DER bytes.
fn fingerprint_hex(cert_der: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(cert_der);
    hex::encode(hasher.finalize())
}

/// Build the internal error the mint/cache paths return.
fn mint_err(msg: impl Into<String>) -> AppError {
    AppError::new(
        StatusCode::INTERNAL_SERVER_ERROR,
        "OFFICE_BRIDGE_CERT_ERROR",
        msg,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// TEST-6 — the minted cert's SAN contains exactly `localhost`,
    /// `127.0.0.1`, and `::1`, CN=localhost, and it round-trips as a valid
    /// rustls server cert.
    #[test]
    fn test6_minted_cert_san_and_rustls_roundtrip() {
        use x509_parser::prelude::*;

        let minted = mint_localhost_cert().expect("mint");

        // Parse the minted DER and inspect the SAN + subject CN.
        let (_, cert) =
            X509Certificate::from_der(&minted.cert_der).expect("parse minted DER");

        // CN=localhost.
        let cn = cert
            .subject()
            .iter_common_name()
            .next()
            .and_then(|a| a.as_str().ok())
            .expect("subject CN present");
        assert_eq!(cn, "localhost", "CN must be localhost");

        // SAN: collect DNS names + IP addresses.
        let san = cert
            .subject_alternative_name()
            .expect("SAN extension parse")
            .expect("SAN extension present");

        let mut dns_names: Vec<String> = Vec::new();
        let mut ips: Vec<Vec<u8>> = Vec::new();
        for name in &san.value.general_names {
            match name {
                GeneralName::DNSName(s) => dns_names.push((*s).to_string()),
                GeneralName::IPAddress(b) => ips.push(b.to_vec()),
                _ => {}
            }
        }

        assert!(
            dns_names.iter().any(|d| d == "localhost"),
            "SAN must contain DNS localhost; got dns={dns_names:?}"
        );
        let v4 = Ipv4Addr::new(127, 0, 0, 1).octets().to_vec();
        let v6 = Ipv6Addr::LOCALHOST.octets().to_vec();
        assert!(
            ips.iter().any(|ip| *ip == v4),
            "SAN must contain IP 127.0.0.1; got ips={ips:?}"
        );
        assert!(
            ips.iter().any(|ip| *ip == v6),
            "SAN must contain IP ::1; got ips={ips:?}"
        );

        // Fingerprint is a 64-char lowercase hex SHA-256.
        assert_eq!(minted.fingerprint.len(), 64);
        assert!(minted.fingerprint.chars().all(|c| c.is_ascii_hexdigit()));

        // Round-trips as a valid single-cert rustls ServerConfig.
        build_server_config(&minted.cert_pem, &minted.key_pem)
            .expect("PEM cert+key load into a rustls ServerConfig");
    }

    /// `load_or_mint` writes a cache on first call and reuses (byte-identical
    /// cert) on the second — proving the trusted cert persists across restarts.
    #[test]
    fn load_or_mint_persists_and_reuses() {
        let dir = tempfile::tempdir().expect("tempdir");
        let first = load_or_mint(dir.path()).expect("first mint");
        assert!(dir.path().join(CERT_FILE).exists());
        assert!(dir.path().join(KEY_FILE).exists());

        let second = load_or_mint(dir.path()).expect("second load");
        // Same cached cert reused (same fingerprint), not re-minted.
        assert_eq!(first.fingerprint, second.fingerprint);
        assert_eq!(first.cert_pem, second.cert_pem);
    }
}
