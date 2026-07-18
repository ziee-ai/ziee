# Chunk BA — GOLDEN SPIKE result (the fail-safe gate)

**Verdict: GOLDEN-CLEAN.** The re-export-shim strategy for the schema-bound
wire types is proven; the byte-identical `types.ts` gate (decision N2) holds.

## What the spike moved
The two cleanly-separable concrete wire types — `User` and `Group`
(`src-app/server/src/modules/user/models.rs`, incl. their `sqlx::FromRow`,
`serde`, `schemars::JsonSchema`, `axum_login::AuthUser` and
`ziee_identity::Principal` impls) — into `ziee-auth` (`crates/ziee-auth/src/models.rs`),
re-exported byte-for-byte by ziee (`modules/user/models.rs` → `pub use ziee_auth::{Group, User};`).

These are the highest-risk types in the whole extraction for the golden gate:
`User` is nested inside `AuthResponse`/`MeResponse` (both stay in ziee), carries a
`#[serde(skip_serializing)] #[schemars(skip)] password_hash`, and is referenced
at ~call sites throughout the crate.

## Result (regen both surfaces vs `.extraction/baseline/`)

| Surface | types.ts | openapi.json |
|---|---|---|
| ui | **BYTE-IDENTICAL** | **CANONICALLY-EQUAL** (`jq -S`) |
| desktop | **BYTE-IDENTICAL** | **CANONICALLY-EQUAL** (`jq -S`) |

Confirms the hypothesis: schemars keys a type by its **short ident** (`User`,
`Group`), not its Rust module path, so moving the crate that *defines* the type
leaves the OpenAPI schema name — and every downstream `$ref` — unchanged. A type
that stays in ziee (`AuthResponse`) but embeds a moved type (`User`) still emits
an identical `$ref: "#/components/schemas/User"`.

## Builds (all exit 0)
- `cargo check -p ziee` — 0 (only pre-existing dead_code warnings)
- `cargo check -p ziee-desktop` — 0
- `cd sdk && cargo check --workspace` — 0 (ziee-auth builds standalone)

## Security angle (mandatory)
`password_hash`'s `#[serde(skip_serializing)] + #[schemars(skip)]` moved verbatim;
the golden byte-identity of `types.ts` positively proves the field is still
absent from the wire schema (no accidental exposure through the crate move). No
new public surface: `ziee-auth` exports only `User`/`Group`, same visibility as
before.
