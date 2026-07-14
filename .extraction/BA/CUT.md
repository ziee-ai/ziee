# Chunk BA — CUT manifest (spike scope only; full chunk STOPPED — see STOP_REPORT.md)

Move the concrete, self-contained wire types `User` + `Group` into `ziee-auth`
(the default, replaceable schema-bound auth crate; depends on `ziee-framework` +
`ziee-identity`), consumed by ziee via an equivalence-preserving re-export shim
(decision N2). This is the golden-verified spike; the remainder of Chunk BA
(auth repos/handlers/providers, `query!` macros, auth-only build DB, migration
relocation, merged migrator) is BLOCKED on N6 de-globalization.

## Files

- move: `src-app/server/src/modules/user/models.rs` → `sdk/crates/ziee-auth/src/models.rs`
  (ziee's `models.rs` RETAINED as a `pub use ziee_auth::{Group, User};` shim; the
   `pub use models::*` in `modules/user/mod.rs` keeps every call site resolving)
- new: `sdk/crates/ziee-auth/src/lib.rs` re-exports `models::{Group, User}`
- edit: `sdk/crates/ziee-auth/Cargo.toml` — deps (serde/schemars/sqlx/chrono/uuid/axum-login + ziee-identity/ziee-framework), versions matched to ziee's catalog
- edit: `src-app/server/Cargo.toml` — add `ziee-auth = { path = "../../sdk/crates/ziee-auth" }` (ziee-side; left uncommitted for orchestrator)

## Symbols
- symbol: `User` (sdk/crates/ziee-auth/src/models.rs) — verbatim, incl. `AuthUser` + `Principal` impls
- symbol: `Group` (sdk/crates/ziee-auth/src/models.rs) — verbatim

## Design-gate
Schema extraction. The gate is E8 golden byte-identity of `types.ts` under a
crate boundary that moves an OpenAPI-registered type. PASSED on both surfaces
(see SPIKE.md). The second design-gate of the full chunk — migration composition
(N3) — is NOT exercised (STOPPED before migration relocation).
