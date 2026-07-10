# PLAN_AUDIT — office-bridge (consolidated)

Per-item verdicts from all five stages (renumbered). Dimension sections retained below.

## Breakage risk
Assessed per stage; no BLOCKED verdicts remain (see per-item lines).

## Pattern conformance
Each stage mirrored its named reference module (see PLAN Patterns).

## Migration collisions
The two office_bridge migrations were renumbered into the desktop `1000…` space (stage: desktop-only, ITEM/DEC noted) — no collision with server migrations.

## OpenAPI regen
The desktop combined spec + `types.ts` are regenerated (mechanical, excluded from the coverage law); the web spec drops all office routes.

## Per-item verdicts

### Stage: Foundation — module, settings, watcher, bridge listener

- **ITEM-1** — verdict: PASS — direct mirror of `web_search/mod.rs` (server-id v5, `#[distributed_slice(MODULE_ENTRIES)]` order 97 free, init kill-switch + spawned idempotent upsert, register_routes merge); probe-gating adds the code_sandbox idiom.
- **ITEM-2** — verdict: PASS — singleton-settings + grant migrations at free numbers 132/133, mirroring web_search 097/098; no collision.
- **ITEM-3** — verdict: PASS — compile-time `PermissionCheck` impls mirroring `web_search/permissions.rs`; `use` granted via migration 133.
- **ITEM-4** — verdict: CONCERN — `rcgen` is a new dep (additive, low risk); must ensure SAN includes `::1` (the proven load-bearing detail) and cert is cached/reused so the trusted cert stays stable across restarts.
- **ITEM-5** — verdict: CONCERN — new standalone rustls WSS listener (axum `ws` + `axum-server`/`rustls` new deps) is novel to ziee (no existing axum WS); dual-stack bind + Origin allowlist + token gate must be right. Isolated (probe-gated), so blast radius is contained; verified by ITEM-5 integration tests.
- **ITEM-6** — verdict: PASS — structural copy of `code_sandbox/backend/mod.rs` (`#[async_trait]` trait + `#[cfg]` `ACTIVE` + `active()`), with a mock impl for tests like `MockTunnelDriver`.
- **ITEM-7** — verdict: CONCERN — the `windows` crate (late-bound IDispatch/`GetActiveObject`/oleacc/EnumWindows) is the largest new surface and the trickiest code; all `#[cfg(windows)]`-scoped so it can't break other targets, and each COM call maps to a spike-proven operation. Elevated `certutil` needs one UAC (approved).
- **ITEM-8** — verdict: PASS — `#[cfg(target_os="macos")]` scaffold + `unsupported` fallback; explicitly UNVERIFIED behind `MAC_TRANSPORT_VERIFIED=false`, so it compiles without claiming correctness (per the plan's macOS gate).
- **ITEM-9** — verdict: PASS — JSON-RPC dispatch mirroring `web_search/handlers.rs`; capability-gating returns typed errors for unsupported host ops (PPT comments/track-changes), matching the proven capability matrix.
- **ITEM-10** — verdict: PASS — chat-extension flag + `#[distributed_slice(CHAT_EXTENSIONS)]` order 29 (<30) + the single `auto_attach_builtin_ids` edit; `is_builtin_server_id` intentionally NOT edited so the mutating tool stays behind approval (matches `control_mcp`'s deliberate absence).
- **ITEM-11** — verdict: CONCERN — adding a `SyncEntity::OfficeDocument` variant in `sync/event.rs` forces an OpenAPI regen and an explicit `Audience::owner` choice at the emit site (never `everyone()` for per-user data); the daemon poll-diff watcher is new but self-contained.
- **ITEM-12** — verdict: PASS — `include_dir!("$CARGO_MANIFEST_DIR/resources/office-bridge")` per `skill/builtin.rs`; assets ported from the proven spike manifest/taskpane.
- **ITEM-13** — verdict: PASS — an admin-gated REST action + settings button; elevation/Office-present detection reuses ITEM-7 platform methods.
- **ITEM-14** — verdict: CONCERN — frontend is shadcn/Radix now (not the antd of prior art); must mirror the CURRENT `literature` panel tri + obey KIT_MANIFEST/design skills; requires ≥1 e2e spec (enumerated in TESTS.md) + `npm run check`/`gate:ui`.
- **ITEM-15** — verdict: PASS — `just openapi-regen` after DTO/permission/sync additions; golden `types_ts_parity` kept green; generated artifacts excluded from coverage law.

### Stage: Pane RPC — daemon↔pane JSON-RPC broker + 5 pane tools

- **ITEM-16** — verdict: PASS — new self-contained `broker.rs`; mirrors `auth.rs` +
- **ITEM-17** — verdict: CONCERN — `handle_socket` rewrite removes the echo, breaking
- **ITEM-18** — verdict: CONCERN — rewiring the 5 pane arms changes the no-pane result
- **ITEM-19** — verdict: PASS — `taskpane.js` extension is additive (adds an inbound
- **ITEM-20** — verdict: PASS — a `WINDOWS_PANE_VERIFICATION.md` manual live checklist
- **ITEM-21** — verdict: PASS — documentation extension of the existing Mac report;

### Stage: run_office_js — open-ended Office.js pane surface

- **ITEM-22** — verdict: PASS — new descriptor mirrors existing pane-tool descriptors; no collision.
- **ITEM-23** — verdict: CONCERN — must ALSO update `settings_mcp_test.rs::EXPECTED_TOOLS` (found in audit); plan amended to include it. No blocker.
- **ITEM-24** — verdict: PASS — slots into the existing host-agnostic pane dispatch seam; approval inherited (DEC-23).
- **ITEM-25** — verdict: PASS — self-contained removal of the arm + struct + 2 tests; drop the now-unused `DocOp` import.
- **ITEM-26** — verdict: CONCERN — wider than one file (trait + 4 impls + 2 tests + `ActResult`); grep confirms no consumer survives, so removal is clean, but it must be done atomically or the crate won't compile. Sequence with ITEM-25.
- **ITEM-27** — verdict: PASS — mirrors `opReadDocument`; `new Function` async-body execution inside `{Word,Excel,PowerPoint}.run` is the standard Office.js embedding; structured error from the Office.js error object.
- **ITEM-28** — verdict: PASS — reuses `capText`/`MAX_READ_CHARS`; pure helper is node-testable via the existing `module.exports` seam.
- **ITEM-29** — verdict: CONCERN — the `OpenDoc` doc-comment reword triggers a DESKTOP `just openapi-regen` (per OpenAPI-regen dimension). Not a blocker; sequenced into phase 8. Pure doc/comment edits elsewhere.
- **ITEM-30** — verdict: PASS — doc-only checklist addition; same `taskpane.js` runs under WebView2, so the Windows step is a live-verify note, mirroring the existing `WINDOWS_PANE_VERIFICATION.md` structure.

### Stage: Desktop-only relocation — module moved server→desktop

- **ITEM-31** — verdict: CONCERN — reshape from `AppModule` to a `host_mount`-style `DesktopModule`; largest code change but fully precedented (resolve exact registration in DEC-31).
- **ITEM-32** — verdict: PASS — pure `crate::`→`ziee::` repath; every referenced symbol is `pub` at its definition.
- **ITEM-33** — verdict: PASS — ~5 crate-root `pub use` additions, all already `pub`; zero private internals exposed.
- **ITEM-34** — verdict: CONCERN — the mcp.rs auto_attach inversion (new `AUTO_ATTACH_BUILTINS` slice); the one genuine ziee→office_bridge decoupling. Solvable, not blocked.
- **ITEM-35** — verdict: PASS — migrations self-contained; next free `…0006/…0007`; post-server ordering safe.
- **ITEM-36** — verdict: PASS — `SyncEntity::OfficeDocument` stays in `ziee` (enum-owned, by-value, drives TS union); no break.
- **ITEM-37** — verdict: PASS — move `resources/office-bridge/` into the desktop crate + update `include_dir!` base.
- **ITEM-38** — verdict: CONCERN — UI module moves to `desktop/ui/`; wire the desktop-ui module registry + drop from web UI (frontend-touching → e2e required).
- **ITEM-39** — verdict: CONCERN — rebase tests onto the desktop `TestServer` harness (host_mount_tests precedent); `settings_mcp_test` uses server test helpers that must exist desktop-side.
- **ITEM-40** — verdict: PASS — dual-spec regen; desktop merge already exists; per-spec parity stays green.
- **ITEM-41** — verdict: PASS — negative proof; `app_builder` self-test auto-excludes; assert server ui spec has no `office` route.
- **ITEM-42** — verdict: PASS — Repos factory removal + 3 call-site rewrites; exact host_mount precedent.
- **ITEM-43** — verdict: PASS — `OfficeBridgeConfig` stays in `ziee::Config`; cosmetic mislabel fix.
- **ITEM-44** — verdict: CONCERN — cross-crate `CHAT_EXTENSIONS` registration unproven; validate first, manual-seam fallback (resolve in DEC-32).

### Stage: Mode-gated approval — read auto-runs, write prompts

- **ITEM-45** — verdict: PASS — descriptor prune mirrors the edit_document removal; unit-test set shrinks to 2.
- **ITEM-46** — verdict: PASS — `mode` is an additive schema field + description copy; no execution change.
- **ITEM-47** — verdict: CONCERN — wider test surgery than one arm (2 arms + PPT pre-gate + const + several unit tests + `test10` removal); grep-confirmed sole users, done atomically so the crate compiles.
- **ITEM-48** — verdict: PASS — pane op removal mirrors the prior feature; shared helpers (`capText`/`ERR_UNSUPPORTED_HOST`) confirmed still used by `run_office_js`.
- **ITEM-49** — verdict: CONCERN — refactors shared hot-path approval code; behavior-preserving extraction gated by exhaustive unit tests + call-site review. No blocker.
- **ITEM-50** — verdict: CONCERN — security-critical classifier; the fail-safe (only exact `"read"` on the office_bridge server bypasses) + spoof test + missing/invalid-mode test lock it.
- **ITEM-51** — verdict: CONCERN — cross-crate test updates (desktop retargets + new server approval-loop integration + id-drift test); enumerated fully in TESTS.md.
- **ITEM-52** — verdict: PASS — doc/comment updates only.
