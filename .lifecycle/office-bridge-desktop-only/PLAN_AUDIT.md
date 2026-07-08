# PLAN_AUDIT — office_bridge desktop-only re-architecture

Audited against the codebase (subagent map of every `crate::` dependency + reverse coupling).
Headline: **feasible, no BLOCKED items.** The public-surface widening is small (all symbols are
already `pub` at definition), and `host_mount` is a working precedent for a desktop-only module
(own routes + `ziee::permissions` types + own repository + migrations in `desktop/tauri/migrations/`).
Two items carry genuine risk (mcp.rs decoupling, cross-crate CHAT_EXTENSIONS) — both solvable and
de-risked by an early validation (ITEM-14).

## Breakage risk

Reverse references to office_bridge OUTSIDE its own dir — each must be handled or `ziee` won't
compile once the module leaves:

- `modules/mod.rs:27` `pub mod office_bridge;` → remove (mechanical).
- `core/repository.rs:229` `Repos` factory field → remove + rewrite 3 call sites to
  `OfficeBridgeRepository::new(pool)` (ITEM-12; exact host_mount precedent). Mechanical.
- `core/config.rs:22` `Option<OfficeBridgeConfig>` → **stays** (inert optional section; ITEM-13). No break.
- `modules/mcp/chat_extension/mcp.rs:172-173` `auto_attach_builtin_ids` hard-refs office_bridge's
  `ATTACH_FLAG` + `office_bridge_server_id()` (+ test at 3062-3095) → the ONE dependency a re-export
  can't fix; needs the AUTO_ATTACH_BUILTINS inversion (ITEM-4). **Main design task, not a blocker.**
- `sync/event.rs:145` `SyncEntity::OfficeDocument` → **stays in ziee** (enum owned by ziee, drives the
  generated frontend TS union; constructed by value, not `#[non_exhaustive]`) — no break (ITEM-6).
- `lib.rs:121/127` test re-exports (`office_bridge_bridge`/`office_bridge_platform`) → remove (tests
  move to desktop, ITEM-9).
- `app_builder.rs:278-296` module-count self-test → auto-correct: it runs in the `ziee` test binary
  which won't link office_bridge, so `MODULE_ENTRIES.len()` legitimately drops by one. No edit needed.

Residual risk: cross-crate linkme collection for CHAT_EXTENSIONS (no precedent) — de-risked by the
ITEM-14 smoke test with a manual-seam fallback.

## Pattern conformance

- Desktop-only module → mirror **`host_mount`** (`desktop/tauri/src/modules/host_mount/`): routes,
  `PermissionCheck` via `ziee::permissions`, own `repository.rs` (`Repository::new(pool)`), migrations
  in `desktop/tauri/migrations/`, hand-registered in `create_desktop_modules`. Direct fit.
- Built-in MCP module SHAPE (settings + tools + chat-ext) → mirror **`web_search`** in the server
  (already the office_bridge template) — unchanged, just relocated + repathed.
- Migrations → mirror `10000000000003_create_remote_access_settings.sql` (1000-space +
  `set_ignore_missing` coexistence) applied by `run_desktop_migrations`.
- Desktop tests → mirror `desktop/tauri/tests/` `host_mount_tests` + the desktop `TestServer` harness.
- New AUTO_ATTACH_BUILTINS slice → mirror the existing `MODULE_ENTRIES`/`CHAT_EXTENSIONS`
  `#[distributed_slice]` definitions.

## Migration collisions

- `ls desktop/tauri/migrations/` ends at `10000000000005`; office_bridge takes **`…0006` + `…0007`**
  — no collision (server uses the disjoint `0000…` space; main's `132_add_openrouter` is irrelevant
  now that office_bridge leaves the server space).
- Both SQL files are self-contained: `create_office_bridge` has no FK/cross-ref; the grant does
  `array_append('office_bridge::use')` onto the Users group row from server migration 1 —
  idempotent + `RAISE WARNING` if absent. Desktop migrations run AFTER all server migrations, so the
  Users row exists → **ordering SAFE**. Cosmetic: fix the "migration 133" mislabel in the comment.

## OpenAPI regen

- This diff touches BOTH `src-app/ui/**` and `src-app/desktop/ui/**` → it IS a frontend change
  (phase 3 needs ≥1 e2e; phase 8 needs `npm run check` for each touched workspace).
- Regenerate BOTH specs: the **web** spec (`ui/openapi/openapi.json` + `ui/src/api-client/types.ts`)
  loses every office_bridge route/type; the **desktop** combined spec (`desktop/ui/…`) keeps them —
  `desktop/tauri/src/openapi.rs` already merges module routes, so no generator change, just regen.
- Parity: `types_ts_parity` + `types_ts_parity_desktop` are per-spec (not cross-spec) — regenerating
  both keeps them green. The regenerated `openapi.json`/`types.ts` are excluded from the phase-6
  coverage law (mechanically generated).

## Per-item verdicts

- **ITEM-1** — verdict: CONCERN — reshape from `AppModule` to a `host_mount`-style `DesktopModule`; largest code change but fully precedented (resolve exact registration in DEC-1).
- **ITEM-2** — verdict: PASS — pure `crate::`→`ziee::` repath; every referenced symbol is `pub` at its definition.
- **ITEM-3** — verdict: PASS — ~5 crate-root `pub use` additions, all already `pub`; zero private internals exposed.
- **ITEM-4** — verdict: CONCERN — the mcp.rs auto_attach inversion (new `AUTO_ATTACH_BUILTINS` slice); the one genuine ziee→office_bridge decoupling. Solvable, not blocked.
- **ITEM-5** — verdict: PASS — migrations self-contained; next free `…0006/…0007`; post-server ordering safe.
- **ITEM-6** — verdict: PASS — `SyncEntity::OfficeDocument` stays in `ziee` (enum-owned, by-value, drives TS union); no break.
- **ITEM-7** — verdict: PASS — move `resources/office-bridge/` into the desktop crate + update `include_dir!` base.
- **ITEM-8** — verdict: CONCERN — UI module moves to `desktop/ui/`; wire the desktop-ui module registry + drop from web UI (frontend-touching → e2e required).
- **ITEM-9** — verdict: CONCERN — rebase tests onto the desktop `TestServer` harness (host_mount_tests precedent); `settings_mcp_test` uses server test helpers that must exist desktop-side.
- **ITEM-10** — verdict: PASS — dual-spec regen; desktop merge already exists; per-spec parity stays green.
- **ITEM-11** — verdict: PASS — negative proof; `app_builder` self-test auto-excludes; assert server ui spec has no `office` route.
- **ITEM-12** — verdict: PASS — Repos factory removal + 3 call-site rewrites; exact host_mount precedent.
- **ITEM-13** — verdict: PASS — `OfficeBridgeConfig` stays in `ziee::Config`; cosmetic mislabel fix.
- **ITEM-14** — verdict: CONCERN — cross-crate `CHAT_EXTENSIONS` registration unproven; validate first, manual-seam fallback (resolve in DEC-2).
