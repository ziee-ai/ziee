# Chunk BG-2 — DRIFT scan (round 1)

Drift = any place the crate move could diverge from pre-move behaviour/surface.
Each candidate reconciled below.

- **DRIFT-1.1** — verdict: none. `url_validator.rs` moved with ZERO source
  changes (no crate-internal deps — only reqwest/url/thiserror/std/tokio). Byte
  diff of the impl body vs the old file = 0. SSRF policy constants, IP-block
  logic, DNS re-resolution, and redirect re-validation are identical.

- **DRIFT-1.2** — verdict: none. `secret.rs` changed exactly three things vs a
  verbatim copy: (a) `crate::common::AppError` → `ziee_core::AppError` (same type,
  re-export collapse); (b) `crate::core::secrets::storage_key()` →
  `crate::secrets::storage_key()` (same global, now framework-local); (c)
  let-chain → nested if-let (TRANSFORMS D3, control-flow identical). No crypto
  call, key-length check, or redaction path touched.

- **DRIFT-1.3** — verdict: none. The `secrets` storage-key global is a process
  static. Moving its declaration crate does not create a second instance — there
  is exactly one `ziee-framework` in the dep graph, and `init_storage_key` (called
  once at boot in `main.rs`/`lib.rs` through the `crate::core::secrets` shim) and
  every `storage_key()` reader (8 module repos + `core/events.rs` +
  `resolve_optional_secret`) resolve to that one static. Same value everywhere.

- **DRIFT-1.4** — verdict: none. `core::outbound` now re-exports from
  `ziee_framework::url_validator` instead of `crate::utils::url_validator`. Both
  paths now resolve to the SAME framework impl (the utils path is a shim), so this
  is a source-of-re-export change with no behavioural effect. The 12 `oauth2`/`apple`
  call sites are byte-unchanged; the `cfg!(debug_assertions)` DEV_LOCAL/PUBLIC
  branch is untouched.

- **DRIFT-1.5** — verdict: none. Wire surface. `SecretView` carries no
  `JsonSchema` impl, so it is absent from every OpenAPI schema; `url_validator`
  and `secrets` expose no handler/DTO. Therefore no schemars ident moved crates,
  and the E8 golden is byte-identical (types.ts, both surfaces) + canonical
  (openapi.json, both surfaces). Machine-verified, then restored via git checkout.

- **DRIFT-1.6** — verdict: none. Feature-unification. `ziee-framework`'s new
  `reqwest` decl matches the server's base `default-features = false` +
  `["rustls-tls","charset","http2"]`, so the unified `src-app/Cargo.lock` gains no
  new default features for the server binary. `cargo check -p ziee` +
  `-p ziee-desktop` exit 0 confirm no build/behaviour change.

- **DRIFT-1.7** — verdict: none. Tests. All 25 `url_validator` + 8 `secret`
  in-source unit tests moved WITH their files and compile/pass under the 2021 SDK
  workspace (`cargo check --workspace` exit 0). No consumer test needed editing
  (no public signature changed — the shims preserve every path).

**Unresolved drifts: 0**
