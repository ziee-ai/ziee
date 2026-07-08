# FIX_ROUND 1 — office-bridge-desktop-only

## Fixes applied (the confirmed phase-6 LEDGER findings)

- **HIGH — chat-extension registration ordering** (`desktop/…/office_bridge/mod.rs`,
  `desktop/…/backend/mod.rs`): the whole office chat integration silently no-op'd because
  `register_office_bridge` pushed the chat extension from the POST-server-start hook, but the chat
  module snapshots the `ExtensionRegistry` (`auto_register_extensions`) DURING init, inside
  `start_server_with_routes` — earlier. **Fix:** split registration into
  `register_office_bridge_static(&config)` (the two pool-free seams — `register_auto_attach_builtin`
  + `register_chat_extension`) called **before** `start_server_with_routes` in `backend/mod.rs`
  (while `config` is still borrowable, before it is moved into the server), and
  `register_office_bridge(&config)` (the pool-dependent MCP-row upsert + bridge listener + watcher)
  kept in the post-start hook. Shared gates (config kill-switch + host probe) factored into
  `office_bridge_enabled()`. Verified: `cargo check -p ziee-desktop` = 0 (42s),
  `cargo test -p ziee-desktop --no-run` = 0.

- **MED — TEST-5 too shallow** (`desktop/…/tests/office_bridge/attach_test.rs`): it exercised the
  runtime registry round-trip only, which is why the ordering bug slipped. **Fix:** rewrote it to
  replicate `auto_register_extensions`' EXACT merge (`CHAT_EXTENSIONS` slice + `runtime_chat_extensions()`,
  sorted by order) and assert the office extension is IN the merged set at order 23 — the precise
  property the bug violated — host-independently (no DB/probe gate).

- **LOW — doc drift** (`PLAN.md` intro): rewrote the stale "linkme distributed_slice" mechanism prose
  to the runtime-seam reality (DEC-1/DEC-2). Non-functional.

## Re-audit (full blind round over `git diff office-bridge-desktop-only-base..HEAD`)

Re-reviewed the whole diff with the fix in place, across correctness / concurrency / patterns-conformance
/ tests-quality / dead-code / api-contract:

- **correctness**: the static seams now run before the registry snapshot; `&config` borrow completes
  before the move into `start_server_with_routes`; the post-start runtime half is unchanged. Correct.
- **patterns-conformance**: the pre-start static push + post-start pool work mirrors how the desktop
  registers other seams; `office_bridge_enabled` centralises the two gates.
- **tests-quality**: TEST-5 now fails if a runtime-registered extension is absent from the merge.
- **concurrency (suspected, LOW — NOT confirmed, pre-existing, boot-once)**: `register_*` append to
  `OnceLock<RwLock<Vec<…>>>`; if `start_backend_server` ever ran twice in one process the extension
  would be double-registered. This risk existed identically in the pre-fix single function and the
  boot path calls it exactly once; the split does not change it. Left as-is (would be a separate,
  cross-cutting hardening, not part of this feature's diff).
- No new leftover `office_bridge` references in `ziee`; no new api-contract change.

**New confirmed findings:** 0
