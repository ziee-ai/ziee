# Chunk C1 — BOUNDARY

What the two new SDK surfaces (`ziee-control-mcp`, `ziee-framework::mcp`) may and
may not name, and why the split keeps the app-agnostic + build-DB-free boundary
clean.

## `ziee-control-mcp` is domain-free AND build-DB-free

- `catalog.rs` deps: `aide::openapi::OpenApi` (ingested into a `serde_json::Value`
  via `to_value`), `serde_json`, `std`, `tracing`. **No** app types, **no** DB,
  **no** `Config`. `init_from_openapi` reads the finished spec **in memory** — it
  never reads the on-disk `openapi.json` and never queries Postgres.
- `policy.rs` deps: the sibling `catalog::Operation` + `serde_json`. Pure
  classification of an `Operation`.
- `tools.rs` deps: `serde_json`. Static descriptors.
- Grep confirms: no `crate::modules`, no `ziee_core`/`ziee_identity`/
  `ziee_framework` domain reference, no `sqlx`, no `query!` in any of the three.

The crate's declared `ziee-framework` + `ziee-identity` deps are **unused in v1**
(retained for the plan §1.2 dependency direction + the v1.5 handler extraction).
That the moved core needs neither is the proof it is a genuinely reusable,
DB-free tool-dispatch **engine**.

## `ziee-framework::mcp` is dependency-free scaffolding

- `JsonRpcRequest`/`Response`/`Error` derive only serde + use `serde_json::Value`;
  `from_app_error` names `ziee_core::AppError` (already a framework dep).
- `loopback_host` is a const-returning fn (the security pin).
- No app type, no DB — it is the shared envelope every built-in MCP server reuses.

## The boundary line — what stayed app-side (v1, decisions N1/N5)

`control_mcp` is **not** build-DB-free: `repository.rs::upsert_builtin_server`
writes an `mcp_servers` row (a `sqlx::query!` against the app schema). So the
module splits on the **DB / app-type boundary**:

- **App-side (stays):** `handlers.rs` (the `RequirePermissions<(ControlUse,)>`
  JWT boundary, the `User`/`Group` per-user permission filter, the reqwest
  forwarded-JWT loopback dispatch, `substitute_path`/`validate_body` request
  hardening, `control_call_needs_approval`), `repository.rs` (the row write —
  the reason the module isn't build-DB-free), `routes.rs`, `permissions.rs`
  (`control::use`), `chat_extension/*`, and the `mod.rs` registration + upsert
  spawn.
- **SDK (moved):** the pure `catalog`/`policy`/`tools` engine + the shared
  JSON-RPC/`loopback_host` scaffolding.

This mirrors every prior chunk's seam: the SDK owns the app-agnostic machinery,
the app owns the config-/schema-bound composition. A fresh app gets the precise
control **engine** now; its self-exposure (the row + routes + chat wiring) is
**v1.5** — it needs the Tier-1 `mcp` registry (N1/N5).

## Second-consumer proof

`sdk/examples/skeleton-server` (framework-only) still `cargo check`s, and the
whole SDK workspace is green (`cargo check --workspace` exit 0), so nothing in the
move reached back into ziee's domain. `ziee-control-mcp` links only framework +
identity (both unused) + aide/serde_json/tracing — no ziee crate.

## Output stays unchanged (E8)

The moved pieces touch no route, schema, permission string, or OpenAPI-visible
type, so ziee's generated `types.ts` is **byte-identical** and `openapi.json`
**canonically-equal** on BOTH ui + desktop — the master invariant that nothing
observable changed.
