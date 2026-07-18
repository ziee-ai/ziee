# Chunk BG-2 — TESTS-MOVED

BG-2 is an equivalence-preserving crate move — no new behaviour, so no NEW tests.
The two files carry their existing in-source `#[cfg(test)]` suites WITH them into
`ziee-framework` (the natural home once the code lives there); the ziee-side shim
files hold no tests (a `pub use` re-export needs none).

## Moved WITH the code into `ziee-framework` (now run under `cargo test -p ziee-framework`)

### `url_validator.rs` — 25 unit tests (verbatim, unchanged)
SSRF policy coverage: `rejects_aws_imds_ip`, `rejects_loopback_v4/v6`,
`rejects_rfc1918_ranges`, `rejects_cgnat_range`, `rejects_ipv4_mapped_loopback`,
`rejects_ipv6_link_local`, `rejects_ipv6_ula`, `rejects_disallowed_schemes_under_strict`,
`rejects_http_under_strict`, `rejects_url_credentials`, `accepts_localhost_when_policy_permits`,
`rejects_localhost_under_strict`, `accepts_known_public_host`, `rejects_url_missing_host`,
`ip_literal_skips_dns`, the `redirect_blocked_reason` set
(`redirect_blocks_disallowed_scheme`, `redirect_blocks_ip_literal_imds_and_private`,
`redirect_blocks_hostname_resolving_to_loopback`, `redirect_allows_public_ip_literal`),
and the `#[tokio::test] guarding_resolver_blocks_loopback_hostname`. These are the
security guard for the SSRF boundary — moved intact, zero assertion change.

### `secret.rs` — 8 unit tests (verbatim, unchanged)
`mask_secret_shows_first_four_then_stars`, `mask_secret_is_char_safe_on_multibyte`,
`secret_view_serializes_redacted`, `secret_view_expose_returns_real_value`,
`secret_view_in_struct_redacts`, `encrypt_secret_errors_when_key_is_too_short`,
`encrypt_secret_returns_none_when_encryption_disabled`. The two `#[tokio::test]`s
use a `connect_lazy` pool and return before touching it (they exercise the
pre-DB key-length + disabled branches) — no build DB needed, keeping
`ziee-framework` build-DB-free at test time too.

## No integration-test follow-through

Unlike BG (which re-signatured `ensure_unique_username`), BG-2 changes NO public
signature — the shims preserve every consumer path (`crate::utils::url_validator::*`,
`crate::common::secret::*`, `crate::core::secrets::*`). So the ziee integration
suites need zero edits. `resolve_optional_secret` / `encrypt_secret` /
`OutboundUrlPolicy` etc. resolve through the shim to the identical framework impl.

**Verification:** `cd sdk && cargo check --workspace` (compiles + type-checks the
moved tests) exit 0; `cargo check -p ziee -p ziee-desktop` exit 0. Full test RUN
is the orchestrator's post-merge step (not part of the BG-2 gate).
