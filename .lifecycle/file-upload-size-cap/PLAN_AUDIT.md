# PLAN_AUDIT — audited against the codebase

## Breakage risk
- **Body-limit const → getter (ITEM-4/5):** the `DefaultBodyLimit::max(...)` layer
  is built at boot inside `file_router()` / the project route builder, which run
  in `build_api_router` (`main.rs:270`) — AFTER the global is set (`main.rs:~187`).
  Read order is safe. Risk: if a boot path builds the router WITHOUT first calling
  `set_max_file_upload_bytes`, the layer falls back to the static's default
  (128 MiB) — a safe default, never a panic. `lib.rs` must set it too (ITEM-3) so
  the harness-spawned binary honors a test override.
- **Handler cap const → getter (ITEM-6):** `MAX_FILE_SIZE` is referenced only in
  `upload_file_inner` (single call site; both `/files/upload` and
  `/projects/{id}/files/upload` funnel through it). No external referencer. Removing
  the `const` breaks nothing else (grep: no other `MAX_FILE_SIZE` users).
- **Raising the cap (50→128 MiB default):** strictly widens what is accepted — no
  previously-accepted upload becomes rejected. The existing 200 MB body-limit
  headroom shrinks to a derived cap+16 MiB, still ≥ the handler cap (invariant holds).
- **Existing test `test_upload_file_too_large` (101 MB):** with the default now
  128 MiB, a 101 MB upload would become 201 — this test WILL break unless
  repurposed onto a small per-test cap (ITEM-11 provides the seam; the test change
  is in the test-plan, TESTS.md). Flagged so it is not a surprise.
- **Frontend constant 100→128 (ITEM-8):** widens client acceptance; no regression.
  The message string changes ("100MB" → "128MB") — any test asserting the old
  string must update (none found in e2e today; the new spec asserts the new text).

## Pattern conformance
- **ITEM-1** mirrors `RateLimitConfig` + `default_rate_limit_*` exactly (`#[serde(default = "fn")]` + free fn returning the default). Conforms.
- **ITEM-2/3** mirror `CACHES_CONFIG`/`SERVER_ADDR` in `app_state.rs` (Lazy<Mutex> + poison-recovering set/get, set-once in `main.rs`). Conforms.
- **ITEM-11** mirrors `refresh_token_expiry_days` (Option numeric field → `{placeholder}` interpolation in the `server:`/`jwt:` YAML block). Conforms.
- **ITEM-8** consolidates 6 duplicated `MAX_FILE_SIZE` consts into one module constant — reduces drift; matches the "single source of truth" preference. Messages keep the antd `message.error` toast pattern already in each component.

## Migration collisions
- **None.** No new SQL migration. Highest existing migration is
  `00000000000132_add_openrouter_provider_type.sql`; this feature adds no
  `migrations/*` file (the cap is YAML config + a process global, not a DB row).

## OpenAPI regen
- **ITEM-1 (config field):** `ServerConfig` derives only `Debug, Deserialize,
  Clone` (NOT `JsonSchema`) → it is server config, not an API schema type →
  **no** openapi.json change.
- **ITEM-7 (doc `.description()` text):** the project-extension
  `upload_and_attach_file_docs` `.description()` currently hardcodes "100 MiB" /
  "over 100 MiB" (`project_extension/handlers.rs:298-303`). Rewriting it to be
  cap-agnostic ("configurable per-file size cap") **changes openapi.json**. A
  clean `cargo run -- --generate-openapi` was verified to produce ONLY this
  one-line content delta (confirmed via a sorted-JSON diff) plus ~157 lines of
  **pre-existing positional key-order churn** unrelated to this change, and the
  operation description does NOT flow into `api-client/types.ts` (types.ts is
  byte-identical before/after regen, for both ui and desktop). Given that, the
  chosen mechanism is a **surgical one-line edit** of the committed
  `ui/openapi/openapi.json` AND `desktop/ui/openapi/openapi.json` (matching the
  source const), which keeps the diff minimal, avoids the incidental reorder, and
  avoids a full Tauri desktop build — while preserving the `types_ts_parity` /
  `types_ts_parity_desktop` golden tests (types.ts unchanged). See DRIFT-1.
  The `upload_file_docs` description (`file/handlers/upload.rs`) cites **no**
  number → left unchanged.

## Per-item verdicts
- **ITEM-1** — verdict: PASS — mirrors `RateLimitConfig` default pattern; `ServerConfig` is not a JsonSchema type, no openapi impact.
- **ITEM-2** — verdict: PASS — mirrors `CACHES_CONFIG` global; default keeps pre-boot/test reads safe.
- **ITEM-3** — verdict: PASS — set-once ordering before `build_api_router` in both `main.rs` and `lib.rs`; fallback default is safe if ever unset.
- **ITEM-4** — verdict: PASS — layer built after the global is set; getter swap is behavior-preserving at the default.
- **ITEM-5** — verdict: PASS — same as ITEM-4 for the project route.
- **ITEM-6** — verdict: PASS — single call site; keeps `FILE_TOO_LARGE`/400; message now states the real limit.
- **ITEM-7** — verdict: CONCERN — requires `just openapi-regen` (both ui + desktop specs) because the doc `.description()` changes; verify content delta and record a drift entry if openapi.json shows a large positional diff.
- **ITEM-8** — verdict: PASS — consolidates duplicated consts; keeps the toast pattern; widens client cap (no regression).
- **ITEM-9** — verdict: PASS — mirrors the existing `${ZIEE_*}` template + allowlist + Dockerfile-ENV pattern; server key is read via serde default when the var is unset.
- **ITEM-10** — verdict: PASS — comment-only; value stays 1024m (≥ derived body limit for any sane cap under ~1 GB).
- **ITEM-11** — verdict: PASS — mirrors `refresh_token_expiry_days`; `None` omits the line so the server default (128) applies.
- **ITEM-12** — verdict: PASS — tests-only; `sniff_mime`/`smuggling_rejection` already return the asserted values (verified in exploration), so these lock in existing behavior with no production change.
