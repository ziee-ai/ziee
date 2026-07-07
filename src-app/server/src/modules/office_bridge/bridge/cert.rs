//! Bridge TLS certificate mint + cache (ITEM-4).
//!
//! The Office task pane is served over `https://localhost:44300` inside Office's
//! WebView2, which refuses an un-trusted cert. We mint an in-process **CA + leaf
//! chain** (rather than a single self-signed CA-as-leaf cert): rustls/webpki
//! rejects a `basicConstraints CA:true` cert used as the end-entity leaf with
//! `CaUsedAsEndEntity`, so the server MUST present a proper NoCa leaf. We install
//! the CA into the OS root store and serve the leaf (signed by that CA), which
//! validates under BOTH rustls (strict) and WebView2/Chromium.
//!
//! - The **CA** (`CN = ziee Office Bridge Local CA`, `basicConstraints CA:true`,
//!   keyCertSign + digitalSignature) is the trust anchor `install_cert_trust`
//!   (ITEM-7) pushes into the OS root store, and what the fingerprint is over.
//! - The **leaf** (`CN = localhost`, SAN = DNS `localhost` + IP `127.0.0.1` +
//!   `::1` — DEC-5, WebView2 resolves `localhost` to `::1`; `NoCa`; EKU
//!   serverAuth) is what the rustls listener serves, presented as `leaf + CA`.
//!
//! Signatures are ECDSA P-256 / SHA-256 (rcgen's default generated key pair).
//! The minted PEM/DER are cached under the app data dir so the *same* trusted CA
//! survives restarts (re-minting would break the installed trust).

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::path::Path;

use axum::http::StatusCode;
use sha2::{Digest, Sha256};

use crate::common::AppError;

/// Cache file names under the app data dir. All four persist so the trusted CA
/// (and the leaf it signs) are re-read on the next boot unchanged.
const CA_CERT_FILE: &str = "office-bridge-ca.pem";
const CA_KEY_FILE: &str = "office-bridge-ca.key.pem";
const LEAF_CERT_FILE: &str = "office-bridge-leaf.pem";
const LEAF_KEY_FILE: &str = "office-bridge-leaf.key.pem";

/// A freshly minted (or loaded-from-cache) bridge CA + leaf chain.
pub struct MintedCert {
    /// PEM encoding of the leaf certificate (CN=localhost, served by rustls).
    pub leaf_cert_pem: String,
    /// PEM encoding of the leaf PKCS#8 private key (fed to rustls + cached).
    /// Never leaves the host.
    pub leaf_key_pem: String,
    /// Full presented chain: leaf PEM followed by CA PEM. This is what the
    /// rustls listener loads as its certificate list, so the server presents
    /// leaf+CA and a client trusting the CA validates the leaf.
    pub chain_pem: String,
    /// PEM encoding of the CA certificate (the trust anchor to install).
    pub ca_pem: String,
    /// DER encoding of the CA certificate — what `install_cert_trust` pushes
    /// into the OS root store, and what the fingerprint is computed over.
    pub ca_der: Vec<u8>,
    /// Lowercase hex SHA-256 of `ca_der` (the installed root) — a stable
    /// identity persisted in `office_bridge_settings.cert_fingerprint` (DEC-8)
    /// and shown in the UI.
    pub fingerprint: String,
}

/// Mint a fresh bridge CA + `localhost` leaf chain (DEC-5).
///
/// CA: `CN = ziee Office Bridge Local CA`, `basicConstraints CA:true`,
/// keyCertSign + digitalSignature. Leaf: `CN = localhost`, SAN = DNS
/// `localhost` + IP `127.0.0.1` + `::1`, `NoCa`, EKU serverAuth, signed BY the
/// CA. ECDSA P-256 / SHA-256.
pub fn mint_localhost_cert() -> Result<MintedCert, AppError> {
    use rcgen::{
        BasicConstraints, CertificateParams, DistinguishedName, DnType,
        ExtendedKeyUsagePurpose, IsCa, KeyPair, KeyUsagePurpose, SanType,
    };

    // ---- CA: self-signed trust anchor -------------------------------------
    let mut ca_params = CertificateParams::default();
    let mut ca_dn = DistinguishedName::new();
    ca_dn.push(DnType::CommonName, "ziee Office Bridge Local CA");
    ca_params.distinguished_name = ca_dn;
    // SECURITY: this CA is minted `BasicConstraints::Unconstrained` and installed
    // into the OS Root store, so anything it signs would be machine-trusted for
    // any host. The safeguard is that the CA private key is DISCARDED right after
    // minting the single `localhost` leaf below — it is never persisted (see
    // `load_or_mint`, which caches only the leaf key + certs and writes an empty
    // CA-key marker) and nothing re-signs in-process, so no further certificate
    // can ever be issued under this anchor. Do not persist or reuse `ca_key`.
    ca_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    ca_params.key_usages = vec![
        KeyUsagePurpose::KeyCertSign,
        KeyUsagePurpose::DigitalSignature,
    ];

    let ca_key = KeyPair::generate().map_err(|e| mint_err(format!("CA key generation: {e}")))?;
    let ca_cert = ca_params
        .self_signed(&ca_key)
        .map_err(|e| mint_err(format!("CA self-sign: {e}")))?;

    // ---- Leaf: CN=localhost, signed by the CA -----------------------------
    let mut leaf_params = CertificateParams::default();
    let mut leaf_dn = DistinguishedName::new();
    leaf_dn.push(DnType::CommonName, "localhost");
    leaf_params.distinguished_name = leaf_dn;

    // SAN: the three names WebView2 may resolve `localhost` to (DEC-5).
    leaf_params.subject_alt_names = vec![
        SanType::DnsName(
            "localhost"
                .try_into()
                .map_err(|e| mint_err(format!("bad DNS SAN: {e}")))?,
        ),
        SanType::IpAddress(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1))),
        SanType::IpAddress(IpAddr::V6(Ipv6Addr::LOCALHOST)),
    ];
    leaf_params.is_ca = IsCa::NoCa;
    leaf_params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];

    let leaf_key =
        KeyPair::generate().map_err(|e| mint_err(format!("leaf key generation: {e}")))?;
    // rcgen 0.13: leaf_params.signed_by(&leaf_public_key, &issuer_cert, &issuer_key)
    let leaf_cert = leaf_params
        .signed_by(&leaf_key, &ca_cert, &ca_key)
        .map_err(|e| mint_err(format!("leaf sign-by-CA: {e}")))?;

    let ca_pem = ca_cert.pem();
    let ca_der = ca_cert.der().to_vec();
    let leaf_cert_pem = leaf_cert.pem();
    let leaf_key_pem = leaf_key.serialize_pem();
    let chain_pem = format!("{leaf_cert_pem}{ca_pem}");
    let fingerprint = fingerprint_hex(&ca_der);

    Ok(MintedCert {
        leaf_cert_pem,
        leaf_key_pem,
        chain_pem,
        ca_pem,
        ca_der,
        fingerprint,
    })
}

/// Return the cached bridge CA + leaf under `data_dir` if present + still
/// parseable, else mint a fresh chain and write it to the cache (so the trusted
/// CA persists across restarts). The cache is the four
/// `<data_dir>/office-bridge-{ca,leaf}{,.key}.pem` files.
pub fn load_or_mint(data_dir: &Path) -> Result<MintedCert, AppError> {
    let ca_cert_path = data_dir.join(CA_CERT_FILE);
    let ca_key_path = data_dir.join(CA_KEY_FILE);
    let leaf_cert_path = data_dir.join(LEAF_CERT_FILE);
    let leaf_key_path = data_dir.join(LEAF_KEY_FILE);

    if ca_cert_path.exists()
        && ca_key_path.exists()
        && leaf_cert_path.exists()
        && leaf_key_path.exists()
    {
        match load_cached(&ca_cert_path, &leaf_cert_path, &leaf_key_path) {
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
    // Cache the CA cert (trust anchor) + the leaf cert & key (what actually
    // serves). The CA private key is never used again after minting — nothing
    // re-signs in-process — so we persist only an empty CA-key marker file to
    // keep the four-file existence check satisfied; the cache reloads (leaf +
    // CA) without re-minting, preserving the installed trust across restarts.
    std::fs::write(&ca_cert_path, &minted.ca_pem)
        .map_err(|e| mint_err(format!("write {}: {e}", ca_cert_path.display())))?;
    std::fs::write(&leaf_cert_path, &minted.leaf_cert_pem)
        .map_err(|e| mint_err(format!("write {}: {e}", leaf_cert_path.display())))?;
    std::fs::write(&leaf_key_path, &minted.leaf_key_pem)
        .map_err(|e| mint_err(format!("write {}: {e}", leaf_key_path.display())))?;
    std::fs::write(&ca_key_path, b"")
        .map_err(|e| mint_err(format!("write {}: {e}", ca_key_path.display())))?;
    Ok(minted)
}

/// Read + validate a cached CA + leaf, reconstructing a [`MintedCert`].
/// Errors (missing PEM blocks, an unparseable key, or a leaf/key that rustls
/// rejects) bubble up so `load_or_mint` re-mints.
fn load_cached(
    ca_cert_path: &Path,
    leaf_cert_path: &Path,
    leaf_key_path: &Path,
) -> Result<MintedCert, AppError> {
    let ca_pem = std::fs::read_to_string(ca_cert_path)
        .map_err(|e| mint_err(format!("read {}: {e}", ca_cert_path.display())))?;
    let leaf_cert_pem = std::fs::read_to_string(leaf_cert_path)
        .map_err(|e| mint_err(format!("read {}: {e}", leaf_cert_path.display())))?;
    let leaf_key_pem = std::fs::read_to_string(leaf_key_path)
        .map_err(|e| mint_err(format!("read {}: {e}", leaf_key_path.display())))?;

    // Parse the CA cert DER out of the PEM (first CERTIFICATE block).
    let ca_der = first_cert_der(&ca_pem)?;
    // The presented chain is leaf followed by CA.
    let chain_pem = format!("{leaf_cert_pem}{ca_pem}");

    // Prove the leaf + key actually load as a rustls server cert before trusting
    // the cache; a pair rustls rejects is treated as unusable (→ re-mint).
    build_server_config(&chain_pem, &leaf_key_pem)?;

    let fingerprint = fingerprint_hex(&ca_der);
    Ok(MintedCert {
        leaf_cert_pem,
        leaf_key_pem,
        chain_pem,
        ca_pem,
        ca_der,
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

/// Build a rustls `ServerConfig` from a PEM cert chain + key. Shared by the
/// cache-validation path here and the ITEM-5 listener. The `cert_pem` should be
/// the full presented chain (leaf followed by CA) so a client trusting the CA
/// validates the served leaf. Uses the `ring` crypto provider explicitly so it
/// does not depend on a process-wide default provider being installed.
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

    /// TEST-6 — the minted LEAF's SAN contains `localhost`, `127.0.0.1`, and
    /// `::1`, CN=localhost; the leaf is NOT a CA while the CA IS a CA; and the
    /// full chain round-trips as a valid rustls server config.
    #[test]
    fn test6_minted_cert_san_and_rustls_roundtrip() {
        use x509_parser::prelude::*;

        let minted = mint_localhost_cert().expect("mint");

        // ---- Leaf: parse + inspect SAN + CN + basicConstraints --------------
        let leaf_der = first_cert_der(&minted.leaf_cert_pem).expect("leaf DER");
        let (_, leaf) = X509Certificate::from_der(&leaf_der).expect("parse leaf DER");

        // CN=localhost.
        let cn = leaf
            .subject()
            .iter_common_name()
            .next()
            .and_then(|a| a.as_str().ok())
            .expect("subject CN present");
        assert_eq!(cn, "localhost", "leaf CN must be localhost");

        // SAN: collect DNS names + IP addresses.
        let san = leaf
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
            "leaf SAN must contain DNS localhost; got dns={dns_names:?}"
        );
        let v4 = Ipv4Addr::new(127, 0, 0, 1).octets().to_vec();
        let v6 = Ipv6Addr::LOCALHOST.octets().to_vec();
        assert!(
            ips.iter().any(|ip| *ip == v4),
            "leaf SAN must contain IP 127.0.0.1; got ips={ips:?}"
        );
        assert!(
            ips.iter().any(|ip| *ip == v6),
            "leaf SAN must contain IP ::1; got ips={ips:?}"
        );

        // Leaf is NOT a CA (basicConstraints CA:false or absent).
        let leaf_bc = leaf.basic_constraints().expect("leaf basicConstraints parse");
        let leaf_is_ca = leaf_bc.map(|bc| bc.value.ca).unwrap_or(false);
        assert!(!leaf_is_ca, "leaf must NOT be a CA (basicConstraints CA:false/absent)");

        // ---- CA: parse + confirm it IS a CA ---------------------------------
        let (_, ca) = X509Certificate::from_der(&minted.ca_der).expect("parse CA DER");
        let ca_bc = ca
            .basic_constraints()
            .expect("CA basicConstraints parse")
            .expect("CA basicConstraints present");
        assert!(ca_bc.value.ca, "CA cert must have basicConstraints CA:true");
        let ca_cn = ca
            .subject()
            .iter_common_name()
            .next()
            .and_then(|a| a.as_str().ok())
            .expect("CA CN present");
        assert_eq!(ca_cn, "ziee Office Bridge Local CA", "CA CN");

        // Fingerprint is a 64-char lowercase hex SHA-256 (of the CA DER).
        assert_eq!(minted.fingerprint.len(), 64);
        assert!(minted.fingerprint.chars().all(|c| c.is_ascii_hexdigit()));
        assert_eq!(minted.fingerprint, fingerprint_hex(&minted.ca_der));

        // ---- The full chain builds a valid rustls ServerConfig --------------
        build_server_config(&minted.chain_pem, &minted.leaf_key_pem)
            .expect("chain PEM + leaf key load into a rustls ServerConfig");
    }

    /// `load_or_mint` writes a cache on first call and reuses (byte-identical
    /// chain) on the second — proving the trusted CA persists across restarts.
    #[test]
    fn load_or_mint_persists_and_reuses() {
        let dir = tempfile::tempdir().expect("tempdir");
        let first = load_or_mint(dir.path()).expect("first mint");
        assert!(dir.path().join(CA_CERT_FILE).exists());
        assert!(dir.path().join(LEAF_CERT_FILE).exists());
        assert!(dir.path().join(LEAF_KEY_FILE).exists());

        let second = load_or_mint(dir.path()).expect("second load");
        // Same cached CA reused (same fingerprint), not re-minted.
        assert_eq!(first.fingerprint, second.fingerprint);
        assert_eq!(first.ca_pem, second.ca_pem);
        assert_eq!(first.leaf_cert_pem, second.leaf_cert_pem);
    }
}
