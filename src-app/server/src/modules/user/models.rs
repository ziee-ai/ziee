// User models — MOVED to `ziee-auth` (Chunk BA spike).
//
// The concrete `User` + `Group` wire types (with their `sqlx::FromRow`,
// `serde`, `schemars::JsonSchema`, `axum_login::AuthUser`, and
// `ziee_identity::Principal` impls) now live in the default auth module crate
// `ziee-auth`. They are re-exported here as an equivalence-preserving shim
// (decision N2) so the ~call sites importing `crate::modules::user::{User,
// Group}` — and, critically, the schemars OpenAPI schema names — are unchanged
// (schemars keys a type by its short ident `User`/`Group`, not its module
// path, so the moved definitions produce byte-identical wire schemas).

pub use ziee_auth::{Group, User};
