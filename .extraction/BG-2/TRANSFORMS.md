# Chunk BG-2 — TRANSFORMS (non-byte-identical changes + rationale)

Every changed line vs a verbatim copy is recorded here with its design decision +
resolution. Zero `TBD` / `TODO` / `ASK`. All transforms are equivalence-preserving;
the E8 golden (types.ts byte-identical + openapi.json canonical, BOTH surfaces) is
the machine proof that no wire surface moved.

---

## Decision 1 — `secret`'s SDK home is `ziee-framework` (RATIFIED); `secrets` global moves with it

`common::secret` depends on the storage-key process-global
(`resolve_optional_secret` reads `crate::core::secrets::storage_key()` at line
144). The move can't change `resolve_optional_secret`'s signature (the ~11
`secret` consumers must stay byte-unchanged via the shim), and it can't reference
the ziee app crate from inside `ziee-framework`.

**Resolution:** move `core::secrets` (the 55-line `OnceCell<Option<String>>`
global + `init_storage_key`/`storage_key`) into `ziee-framework` alongside the
crypto, and have `secret::resolve_optional_secret` read `crate::secrets::storage_key()`
(framework-local). A `OnceCell` static is ONE instance per process regardless of
which crate declares it (there is exactly one `ziee-framework` in the dep graph);
`init_storage_key` is still called once at boot (`main.rs`/`lib.rs`, via the
`crate::core::secrets` shim → the framework global) and every `storage_key()` read
observes the same value. Behaviour byte-identical. `secrets` is pure infra (an
at-rest-key holder), the natural sibling of the crypto that consumes it — not a
domain global, so this does not violate "no domain coupling in the framework."
It stays build-DB-free (`once_cell` only).

## Decision 2 — `AppError` source repoint (`secret.rs`)

`secret.rs` named `crate::common::AppError` (a re-export of `ziee_core::AppError`).

**Resolution:** in the framework copy, `use ziee_core::AppError;`
(`ziee-framework` already depends on `ziee-core` and uses `ziee_core::AppError`
elsewhere, e.g. `mcp.rs`). `AppError::internal_error` / `AppError::database_error`
are unchanged — same type, same variants, same messages. No behavioural or wire
change (`AppError` is not in the moved file's public surface as a new type).

## Decision 3 — edition-2024 let-chains → nested `if let` (`secret.rs`, `secrets.rs`)

The SDK workspace (`sdk/Cargo.toml`) is edition 2021; the ziee server crate is
edition 2024. Two let-chains in the moved code (`resolve_optional_secret`'s
`if let … && let …`, and `init_storage_key`'s `if let … && prev != key`) are a
2024-only syntax and fail to compile under 2021.

**Resolution:** rewrite each let-chain as the exactly-equivalent nested `if let`
(same short-circuit order, same branches, same side effects). This is a pure
syntactic desugar with identical control flow — the same mechanical downgrade
prior chunks applied when moving 2024 code into the 2021 SDK workspace. A
one-line comment marks each. Verified: framework unit tests
(`encrypt_secret_*`, `secret_view_*`, `mask_secret_*`) compile + pass under 2021.

## Decision 4 — `core::outbound` retargets `ziee_framework::url_validator` (completes BG Decision 5)

BG left `core/outbound.rs` re-exporting `crate::utils::url_validator::{…}` and
documented that "when `url_validator` lands in `ziee-framework`, `ziee-auth`
retargets this import at the framework crate."

**Resolution:** with `url_validator` now in `ziee-framework`, `core/outbound.rs`
re-exports `ziee_framework::url_validator::{OutboundUrlPolicy,
build_validated_client, validate_outbound_url}` directly. `auth::providers::{oauth2,
apple}` keep naming `crate::core::outbound::…` (12 sites, byte-unchanged). Same three
functions, same `cfg!(debug_assertions)` DEV_LOCAL/PUBLIC policy branch — SSRF
behaviour byte-identical. (`crate::utils::url_validator` is retained as a shim for
the other ~16 consumers; both shim and `core::outbound` now resolve to the one
framework impl.)

## Decision 5 — `reqwest` feature parity to preserve the server build

Adding `reqwest` to `ziee-framework` risks flipping default features on in the
unified `src-app/Cargo.lock` (cargo feature-unification is additive; a
`default-features = true` decl would enable defaults graph-wide).

**Resolution:** declare `reqwest = { version = "0.12", default-features = false,
features = ["rustls-tls", "charset", "http2"] }` — byte-matching the ziee server's
base `[workspace.dependencies]` decl. The DNS-resolver (`reqwest::dns::Resolve`) +
redirect (`reqwest::redirect::Policy`) surface `url_validator` uses is in reqwest
core, behind no extra feature. E8 golden (both surfaces) + `cargo check -p ziee`
exit 0 confirm the server build is unaffected.
