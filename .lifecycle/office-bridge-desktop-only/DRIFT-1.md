# DRIFT-1 — implementation vs plan

Audited the full implementation (`git diff office-bridge-desktop-only-base..HEAD`, 10 commits) against
PLAN.md + DECISIONS.md.

- **DRIFT-1.1** — verdict: resolved — DEC-1/DEC-2 registration flipped from cross-crate
  `#[distributed_slice(ziee::…)]` to the RUNTIME SEAM (office_bridge is a `DesktopModule`;
  `register_office_bridge(&Config)` runs post-server-start in `backend/mod.rs:348` and calls
  `ziee::chat_extension::register_chat_extension` + `ziee::register_auto_attach_builtin`). PLAN
  ITEM-1/14 + DECISIONS DEC-1/DEC-2 were amended to this before implementation; codebase precedent is
  `code_sandbox::register_sandbox_mount_provider` (host_mount). Implemented as amended.
- **DRIFT-1.2** — verdict: impl-wins — ITEM-8 planned "wire into the desktop-ui module registry";
  the actual desktop UI discovery is **glob-driven** (`desktop-loader.ts` auto-picks
  `desktop/ui/src/modules/office-bridge`; the web-ui glob drops it) so NO registry edits are needed —
  only the gallery coverage/state entries move web→desktop. PLAN ITEM-8 amended to reflect glob
  discovery.
- **DRIFT-1.3** — verdict: resolved — ITEM-3 facade widening implemented as planned; additionally
  `pub use core::config::OfficeBridgeConfig` was added (for TEST-14) — still an already-`pub` type,
  within ITEM-3's "widen the crate-root facade with already-pub symbols" scope. No private internal
  exposed.
- **DRIFT-1.4** — verdict: none — a feared desktop HTTP-test-harness gap (raised mid-implementation)
  does NOT exist: the desktop test crate `#[path]`-includes the shared `harness_inner.rs`
  (`integration_tests.rs:16`), giving it `TestServer::start_desktop()` + `test_helpers`. ITEM-9 tests
  moved with a `start()`→`start_desktop()` flip + a `ziee::office_bridge_*` →
  `ziee_desktop::modules::office_bridge::*` repath (+ 4 desktop test dev-deps) — no harness port.
- **DRIFT-1.5** — verdict: none — all remaining items implemented as planned: module + migrations
  (→ desktop `1000…0006/0007`) + assets git-moved; the 7 `Repos.office_bridge` sites rewritten to
  `OfficeBridgeRepository::new(pool)` + factory field removed; `SyncEntity::OfficeDocument` +
  `OfficeBridgeConfig` retained in `ziee`; `mcp.rs` inverted; both OpenAPI specs regenerated (web lost
  office_bridge, desktop kept it); TEST-16 server-negative proof added. `cargo check`/`cargo test
  --no-run` green for BOTH `ziee` (no office_bridge) and `ziee-desktop` (with it); `npm run check`
  green in both UI workspaces.

**Unresolved drifts:** 0
