//! Dedicated hardened HTTP client for the api-key-bearing provider
//! file-upload path (`llm_provider_files`).
//!
//! The shared chat/embeddings client (`super::http_client()`) is deliberately
//! NOT reused here. A file upload POSTs the file bytes AND the provider's
//! api_key to an admin-configured `base_url`. The server pre-flight-validates
//! that URL (`utils::url_validator::validate_outbound_url` in
//! `llm_provider_files::service`), but that check resolves DNS *once*, at
//! handler time — leaving a DNS-rebinding window: a hostname that resolved to a
//! public IP during the pre-flight check can rebind to loopback / RFC1918 /
//! link-local (cloud IMDS `169.254.169.254`) before `reqwest` actually opens
//! the socket, and the secret leaves the process to an internal target.
//!
//! This client installs a connect-time [`GuardingResolver`] that re-checks
//! EVERY resolved address at connect time and refuses the whole resolution if
//! any address is private/loopback/link-local — so there is no gap between the
//! check and the connect. `.no_proxy()` additionally prevents an ambient
//! `HTTP(S)_PROXY` env var from tunnelling the secret-bearing request through an
//! unvalidated proxy that would bypass the resolver entirely. This closes the
//! 6-002 residual SSRF (DNS-rebind window) on the upload path.
//!
//! The address policy is fixed (public IPs only), mirroring the
//! `OutboundUrlPolicy::PUBLIC_HTTP_OR_HTTPS` used by the server-side pre-flight
//! check. This module is self-contained rather than reusing the server's
//! `utils::url_validator` because `ai-providers` is a leaf crate that the
//! `ziee` server crate depends on, not the other way round.

use reqwest::dns::{Addrs, Name, Resolve, Resolving};
use reqwest::Client;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, ToSocketAddrs};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

/// SSRF-hardened HTTP client for the file-upload path. Built once (TLS session +
/// connection pool reused process-wide) with the connect-time DNS-rebind guard,
/// proxies disabled, and the same key-leak-safe timeout/redirect defaults as the
/// shared chat client.
///
/// - `dns_resolver(GuardingResolver)` — re-validates every resolved address at
///   connect time, closing the DNS-rebind window left by the one-shot
///   pre-flight check (this is the whole point of the dedicated client).
/// - `no_proxy()` — the api_key-bearing request must not be tunnelled through
///   an ambient `HTTP(S)_PROXY`, which would connect to an arbitrary host and
///   sidestep the resolver.
/// - `redirect(none)` — provider Files APIs never legitimately 30x, and reqwest
///   only strips `Authorization` on cross-host redirects; the custom
///   `x-api-key` / `x-goog-api-key` headers would otherwise follow a redirect to
///   an attacker host and leak the key.
/// - `connect_timeout` / `read_timeout` — mirror the shared client so a
///   blackholed or slow-loris upload endpoint fails fast instead of pinning a
///   task + socket forever.
pub(crate) fn upload_http_client() -> Client {
    static CLIENT: OnceLock<Client> = OnceLock::new();
    CLIENT
        .get_or_init(|| {
            Client::builder()
                .dns_resolver(Arc::new(GuardingResolver))
                .no_proxy()
                .connect_timeout(Duration::from_secs(10))
                .read_timeout(Duration::from_secs(600))
                .redirect(reqwest::redirect::Policy::none())
                .build()
                // Builder only fails on TLS-backend init; fall back to a guarded
                // resolver-only client so the upload still can't reach an
                // internal target (never a plain unguarded `Client::new()`).
                .unwrap_or_else(|_| {
                    Client::builder()
                        .dns_resolver(Arc::new(GuardingResolver))
                        .no_proxy()
                        .redirect(reqwest::redirect::Policy::none())
                        .build()
                        .expect("guarded upload client build")
                })
        })
        .clone()
}

/// A reqwest DNS resolver that filters resolved addresses through the fixed
/// public-only SSRF policy at connect time. Rejects the WHOLE resolution if ANY
/// returned address is private/loopback/link-local (split-horizon paranoia), so
/// an attacker cannot rebind a hostname to a blocked IP between the server's
/// pre-flight check and the socket connect.
#[derive(Debug, Clone, Copy)]
struct GuardingResolver;

impl Resolve for GuardingResolver {
    fn resolve(&self, name: Name) -> Resolving {
        let host = name.as_str().to_string();
        Box::pin(async move {
            let host_for_resolve = host.clone();
            // Blocking system resolution, off the async runtime. Port 0 — the
            // connector overrides it with the request's real port.
            let resolved: Vec<SocketAddr> = tokio::task::spawn_blocking(move || {
                (host_for_resolve.as_str(), 0u16)
                    .to_socket_addrs()
                    .map(|it| it.collect::<Vec<_>>())
            })
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

            if let Some(bad) = resolved.iter().find(|s| is_blocked_ip(&s.ip())) {
                return Err(format!(
                    "SSRF policy blocked '{host}' resolving to {}",
                    bad.ip()
                )
                .into());
            }
            Ok(Box::new(resolved.into_iter()) as Addrs)
        })
    }
}

/// Public-only address policy (equivalent to the server's
/// `OutboundUrlPolicy::PUBLIC_HTTP_OR_HTTPS`): blocks loopback, RFC1918
/// private, CGNAT, link-local (incl. cloud IMDS `169.254.169.254`), ULA,
/// multicast, broadcast, documentation and unspecified addresses.
fn is_blocked_ip(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => is_blocked_v4(v4),
        IpAddr::V6(v6) => is_blocked_v6(v6),
    }
}

fn is_blocked_v4(ip: &Ipv4Addr) -> bool {
    if ip.is_loopback()
        || ip.is_link_local()
        || ip.is_private()
        || ip.is_broadcast()
        || ip.is_documentation()
        || ip.is_unspecified()
        || ip.is_multicast()
    {
        return true;
    }
    // CGNAT 100.64.0.0/10 (RFC 6598) — no stdlib helper; test octets directly.
    let oct = ip.octets();
    oct[0] == 100 && (64..=127).contains(&oct[1])
}

fn is_blocked_v6(ip: &Ipv6Addr) -> bool {
    if ip.is_loopback() || ip.is_unspecified() || ip.is_multicast() {
        return true;
    }
    let segments = ip.segments();
    // Link-local fe80::/10
    if segments[0] & 0xffc0 == 0xfe80 {
        return true;
    }
    // Unique local fc00::/7
    if segments[0] & 0xfe00 == 0xfc00 {
        return true;
    }
    // IPv4-mapped ::ffff:0:0/96 — unwrap and recurse.
    if let Some(v4) = ip.to_ipv4_mapped() {
        return is_blocked_v4(&v4);
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_aws_imds_link_local() {
        assert!(is_blocked_ip(&"169.254.169.254".parse().unwrap()));
    }

    #[test]
    fn blocks_loopback() {
        assert!(is_blocked_ip(&"127.0.0.1".parse().unwrap()));
        assert!(is_blocked_ip(&"::1".parse().unwrap()));
    }

    #[test]
    fn blocks_rfc1918_private() {
        for ip in [
            "10.0.0.1",
            "10.255.255.255",
            "172.16.0.1",
            "172.31.255.255",
            "192.168.0.1",
        ] {
            assert!(is_blocked_ip(&ip.parse().unwrap()), "{ip} must be blocked");
        }
    }

    #[test]
    fn blocks_cgnat() {
        assert!(is_blocked_ip(&"100.64.0.1".parse().unwrap()));
        assert!(is_blocked_ip(&"100.127.255.255".parse().unwrap()));
        // Just outside CGNAT is public.
        assert!(!is_blocked_ip(&"100.63.255.255".parse().unwrap()));
        assert!(!is_blocked_ip(&"100.128.0.0".parse().unwrap()));
    }

    #[test]
    fn blocks_unspecified_and_multicast() {
        assert!(is_blocked_ip(&"0.0.0.0".parse().unwrap()));
        assert!(is_blocked_ip(&"224.0.0.1".parse().unwrap()));
    }

    #[test]
    fn blocks_ula_and_v6_link_local() {
        assert!(is_blocked_ip(&"fc00::1".parse().unwrap()));
        assert!(is_blocked_ip(&"fe80::1".parse().unwrap()));
    }

    #[test]
    fn blocks_ipv4_mapped_v6_loopback() {
        assert!(is_blocked_ip(&"::ffff:127.0.0.1".parse().unwrap()));
        assert!(is_blocked_ip(&"::ffff:169.254.169.254".parse().unwrap()));
    }

    #[test]
    fn allows_public_addresses() {
        assert!(!is_blocked_ip(&"1.1.1.1".parse().unwrap()));
        assert!(!is_blocked_ip(&"8.8.8.8".parse().unwrap()));
        assert!(!is_blocked_ip(&"93.184.216.34".parse().unwrap()));
        assert!(!is_blocked_ip(&"2606:4700:4700::1111".parse().unwrap()));
    }

    #[test]
    fn upload_client_builds() {
        // Smoke: the guarded client constructs without panicking.
        let _ = upload_http_client();
    }
}
