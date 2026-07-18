# Chunk C1 — TRANSFORMS

Every transform applied moving the control tool-dispatch core into
`ziee-control-mcp` and the shared MCP scaffolding into `ziee-framework`, each
with its design decision + resolution. Zero TBD.

## T-1 — `policy.rs` + `tools.rs` moved BYTE-FOR-BYTE

`policy.rs` (the reachability/mutation denylist: `is_denied`, `is_mutating`,
`DENY_PATH_PREFIXES`, the SSE / token / byte-stream segment guards + 3 tests) and
`tools.rs` (the three static tool descriptors + the tool-name consts) are pure —
`policy` names only `super::catalog::Operation` (a sibling module in the same new
crate, so the `super::` path resolves unchanged) + `serde_json::Value`; `tools`
names only `serde_json`. Neither touches a DB, app types, or the framework.

**Resolution:** copied via `cp`; `diff <(git show HEAD:…/policy.rs) sdk/…/policy.rs`
and the same for `tools.rs` are **empty** (exit 0). No logic/ordering/text change.

## T-2 — `catalog.rs` moved verbatim EXCEPT two edition-2024 let-chains lowered

### Decision — the SDK workspace is edition 2021; catalog.rs used let-chains

The ziee server crate is edition **2024**; the SDK workspace (and every prior SDK
crate — B1..B6) is edition **2021**. `catalog.rs::schema_has_secret_field_rec`
used two `if let … && <cond> { }` **let-chains** (an edition-2024 feature). A
byte-verbatim move fails to compile under 2021 (`error: let chains are only
allowed in Rust 2024 or later`). B6's moved `emit_ts.rs` compiled under 2021
because its ziee original was already written in nested-`if let` form; catalog.rs
was not. Two options: (a) make this one crate edition 2024, or (b) lower the two
let-chains to the semantically-identical nested-`if let` form. Since the ziee
`catalog.rs` is **deleted** (the SDK copy is now the sole source), there is no
runtime "byte-identical to ziee" property to preserve — only **semantic**
equivalence. Keeping the whole SDK workspace uniformly edition 2021 (matching
B1..B6) is the lower-surprise choice.

**Resolution:** lowered both let-chains — `if let Some(x) = e && cond { body }`
→ `if let Some(x) = e { if cond { body } }` — which is exactly how Rust desugars a
2-term let-chain, so behavior is identical (a short-circuit AND with the same
early-`return true`). A one-line comment marks each. `diff` shows **only** these
two edits; the other ~548 lines (incl. all 7 unit tests) are byte-identical. The
7 tests pass in the SDK (`cargo test -p ziee-control-mcp`), including
`detects_secret_request_field` which exercises the exact `items` + `anyOf`/`oneOf`/
`allOf` recursion paths these let-chains implement.

## T-3 — JSON-RPC scaffolding → `ziee-framework::mcp` (one type-path transform)

### Decision — move the shared MCP scaffolding, keep every call site unchanged

Plan §3 C1 + §7: "the shared MCP-server scaffolding (JSON-RPC types +
`loopback_host`, dependency-free) moves into `ziee-framework`." Verified
dependency-free: `JsonRpcRequest`/`Response`/`Error` derive only serde and use
`serde_json::Value`; `JsonRpcError::from_app_error` names `AppError` (in
`ziee-core`, a framework dep); `loopback_host` is a const-returning fn. They are
**not** yet in the framework. The types live today in `code_sandbox::types` and
are imported by **13** built-in MCP handlers as `code_sandbox::types::JsonRpc*`;
`loopback_host` lives in `code_sandbox::mod` and is called by **15** modules as
`code_sandbox::loopback_host(...)`. A move must not rewrite those 28 call sites.

**Resolution:** the types + fn move verbatim into `ziee_framework::mcp`, with the
**single** production change `crate::common::AppError` → `ziee_core::AppError` in
`from_app_error` (in ziee `crate::common::AppError` is itself a B1 re-export of
`ziee_core::AppError`, so the referenced `status_code()`/`error_code()`/
`to_string()` surface is identical). ziee then re-exports through two shims:
`code_sandbox::types` does `pub use ziee_framework::mcp::{JsonRpcError,
JsonRpcRequest, JsonRpcResponse};` and `code_sandbox::mod` does `pub use
ziee_framework::mcp::loopback_host;`. All 28 call sites resolve unchanged. The 5
JSON-RPC + 2 `loopback_host` tests move to `mcp.rs` (7 pass in the framework); the
now-unused `serde::{Deserialize, Serialize}` import is dropped from
`code_sandbox::types` (the JSON-RPC block was its only user, so `-D warnings`
stays clean).

## T-4 — control-core re-export shim in ziee (no call-site rewrites)

### Decision — how `super::catalog/policy/tools` + the two boot sites keep resolving

The retained app-side `control_mcp/handlers.rs` uses `super::catalog::{self,
ControlCatalog, Operation}`, `super::policy`, `super::tools`; the two OpenAPI boot
sites (`lib.rs:540`, `main.rs:347`) call
`crate::modules::control_mcp::catalog::init_from_openapi(&api_doc)`. After the move
these three modules no longer exist under `control_mcp/`.

**Resolution:** `control_mcp/mod.rs` replaces the three `pub mod …;` with a single
`pub use ziee_control_mcp::{catalog, policy, tools};`. A `pub use` of a module
makes `control_mcp::catalog` (and thus `super::catalog` from `handlers.rs`) an
alias for `ziee_control_mcp::catalog`, so `super::catalog::{self, ControlCatalog,
Operation}`, `catalog::catalog()`, `catalog::build_catalog(…)` (in the retained
handler tests), `policy::is_mutating/is_denied`, `tools::LIST_CAPABILITIES/…`, and
`control_mcp::catalog::init_from_openapi` all resolve unchanged. `ControlCatalog`
+ `Operation` cross the crate boundary via their **already-`pub`** methods/fields,
so no visibility change is needed. Zero call-site edits outside `mod.rs` +
`Cargo.toml`.

## T-5 — `ziee-control-mcp` deps

### Decision — what the tool-dispatch core needs, and why framework/identity stay

`catalog::init_from_openapi` takes `&aide::openapi::OpenApi` (→ `aide`); the pure
builder + tool descriptors use `serde_json` (`Value`/`json!`); catalog init logs
via `tracing`. The moved core does **not** name `ziee-framework` or
`ziee-identity` in v1 (the permission filter that uses the identity resolver stays
in the app-side handler).

**Resolution:** added `aide` (feature-matched to the framework's), `serde_json
= "1.0.141"`, `tracing = "0.1"` — versions matching the ziee server catalog so the
single `src-app/Cargo.lock` unifies them (no duplicate build). Retained the
declared `ziee-framework` + `ziee-identity` path deps per the plan §1.2 dependency
direction (framework + identity) and because the **v1.5** extraction of the
handler / permission filter needs the injected `IdentityResolver` +
`PermissionCheck` traits. Unused crate deps are allow-by-default (the workspace
does not enable `unused_crate_dependencies`), so `cargo check --workspace` stays
green — proven by the placeholder crate having carried both deps unused since
Chunk 0.

## Decision — the DB-free-core vs app-side-registration split (N1/N5)

The single load-bearing decision of C1: **which of `control_mcp`'s eight files
move.**

`control_mcp` is **not stateless** — `init` writes an `mcp_servers` row via
`repository.rs::upsert_builtin_server` (a `sqlx::query!` against the app schema).
An app-agnostic, build-DB-free SDK crate cannot hold that write (its `query!`
would need the app's build DB). So the module splits along the **DB / app-type
boundary**:

- **MOVES (build-DB-free, pure):** `catalog.rs` (reads the in-memory OpenAPI doc),
  `policy.rs` (classifies an `Operation`), `tools.rs` (static descriptors). None
  touch a DB or an app type.
- **STAYS app-side (v1):** `repository.rs` (the `mcp_servers` **row write** — the
  reason the full module isn't build-DB-free), `handlers.rs` (holds the
  `RequirePermissions<(ControlUse,)>` JWT boundary + the `User`/`Group` permission
  filter + the reqwest forwarded-JWT loopback dispatch — all app types),
  `routes.rs`, `permissions.rs`, `chat_extension/*`, and the `mod.rs` module
  registration + `mcp_servers` upsert spawn.

**Resolution:** the CUT manifest moves exactly `{catalog, policy, tools}.rs` into
`ziee-control-mcp` and lifts the shared JSON-RPC/`loopback_host` scaffolding into
`ziee-framework`; everything else in `control_mcp/` is untouched except the
`mod.rs` re-export shim. A NEW app therefore gets the precise, reusable
tool-dispatch **engine** now, and its self-exposure (the `mcp_servers` row +
routes + chat wiring) is **descoped to v1.5** — it needs the Tier-1 `mcp` registry
(decisions N1/N5). This matches the plan §3/§7 wording exactly.
