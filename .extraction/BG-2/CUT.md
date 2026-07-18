# Chunk BG-2 — move `url_validator` (SSRF) + `secret` crypto into `ziee-framework` (CUT manifest)

**This IS a crate move** (unlike BG, which was in-ziee de-globalization). BG-2
lands the two shared-infra prerequisites BA-full needs into the SDK, so
`ziee-auth` can name them from `ziee-framework` instead of the ziee app crate:

1. **`url_validator`** — the domain-free SSRF outbound-URL guard
   (`OutboundUrlPolicy` + `validate_outbound_url` + `build_validated_client` +
   `GuardingResolver` + redirect re-validation). BG Decision 5 explicitly
   DEFERRED this move ("when `url_validator` lands in `ziee-framework`,
   `ziee-auth` retargets the single `core::outbound` import"). BG-2 does the move;
   `core::outbound` now points at `ziee_framework::url_validator`.
2. **`secret`** — the at-rest secret crypto (`encrypt_secret` / `decrypt_secret` /
   `resolve_optional_secret` + `SecretView` / `mask_secret`). RATIFIED SDK home =
   `ziee-framework`. Build-DB-free: it uses runtime `sqlx::query_as("SELECT
   pgp_sym_encrypt($1,$2)")`, NEVER a compile-time `query!` — confirmed by grep
   (zero `query!`/`query_as!`/`query_scalar!` in the file), so `ziee-framework`
   stays build-DB-free.
3. **`secrets`** (companion of #2) — the at-rest storage-key process-global
   (`init_storage_key` / `storage_key`, a `OnceCell<Option<String>>`).
   `resolve_optional_secret` reads it, and the ~11 `secret` consumers plus the ~13
   `core::secrets` consumers must stay byte-unchanged, so the global moves WITH the
   crypto rather than mutating `resolve_optional_secret`'s signature. It is ONE
   static per process regardless of owning crate → behaviour identical.

Everything is **equivalence-preserving** (behaviour byte-identical; no wire
change — E8 golden identical on BOTH surfaces). ziee consumes all three via
re-export shims, so every consumer's `use` path is unchanged.

## Consumers preserved by the shims (all unchanged)

| Moved item | ziee shim (kept path) | # consumers |
|---|---|---|
| `ziee_framework::url_validator::*` | `crate::utils::url_validator::*` | 16 files (+ `core::outbound`) |
| `ziee_framework::secret::*` | `crate::common::secret::*` | 16 files |
| `ziee_framework::secrets::*` | `crate::core::secrets::*` | 13 sites (main/lib boot + 8 module repos + core/events) |

## Files — SDK submodule (`sdk/`)

### NEW (3)
- `crates/ziee-framework/src/url_validator.rs` — verbatim move (zero
  crate-internal deps: only `reqwest`/`url`/`thiserror`/`std`/`tokio`). Its 25
  in-source unit tests move with it.
- `crates/ziee-framework/src/secret.rs` — move + 2 name edits (`crate::common::AppError`
  → `ziee_core::AppError`; `crate::core::secrets::storage_key()` →
  `crate::secrets::storage_key()`) + one let-chain desugar (see TRANSFORMS D3). Its
  8 in-source unit tests move with it.
- `crates/ziee-framework/src/secrets.rs` — verbatim move + one let-chain desugar
  (TRANSFORMS D3).

### MODIFIED (3)
- `crates/ziee-framework/src/lib.rs` — `pub mod {secret, secrets, url_validator};` + doc.
- `crates/ziee-framework/Cargo.toml` — add `reqwest` (matching the server base
  decl: `default-features=false` + rustls-tls, so feature-unification doesn't flip
  server defaults), `url`, `thiserror`, `once_cell`; `tokio` gains `rt` (spawn_blocking)
  + `macros` (the `#[tokio::test]`s). Versions match the ziee server catalog.
- `Cargo.lock` — regenerated (new edges into ziee-framework).

## Files — ziee app side (`src-app/`)

### MODIFIED (5)
- `server/src/utils/url_validator.rs` — 608-line impl replaced by
  `pub use ziee_framework::url_validator::*;` shim.
- `server/src/common/secret.rs` — 263-line impl replaced by
  `pub use ziee_framework::secret::*;` shim.
- `server/src/core/secrets.rs` — 55-line global replaced by
  `pub use ziee_framework::secrets::*;` shim.
- `server/src/core/outbound.rs` — the 3-symbol re-export now sources from
  `ziee_framework::url_validator` (was `crate::utils::url_validator`), completing
  BG Decision 5.
- `Cargo.lock` — regenerated.

`utils/mod.rs`, `common/mod.rs`, `core/mod.rs` are UNCHANGED (each still declares
`pub mod {url_validator,secret,secrets};` — the module file is now a shim).
