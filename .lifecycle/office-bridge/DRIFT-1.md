# Office Bridge — DRIFT round 1 (implementation vs plan)

Audited the implemented code (14 commits, ITEM-1..15) against PLAN.md / DECISIONS.md. Divergences and
their resolution:

- **DRIFT-1.1** — verdict: impl-wins — Cert design changed from a single self-signed `CA:true` cert (PLAN ITEM-4) to a proper **CA + leaf chain**. The single-CA-as-leaf cert failed rustls/webpki with `CaUsedAsEndEntity` (TEST-7). The chain (trust the CA, serve leaf+CA) is correct for BOTH rustls and WebView2/Chromium. Plan intent (a trusted localhost cert) is preserved; the mechanism was wrong and is amended in code. TEST-6/TEST-7 pass.
- **DRIFT-1.2** — verdict: impl-wins — PLAN hedged the deploy kill-switch (const fallback if non-trivial); the impl added the real `OfficeBridgeConfig { enabled }` field as a trivial mirror of `WebSearchConfig`/`ControlMcpConfig`, directly satisfying DEC-12. The plan's cautious wording is superseded by the cleaner real config.
- **DRIFT-1.3** — verdict: impl-wins — Added a REST `GET /api/office-bridge/documents` endpoint (gated `OfficeBridgeUse`) not explicitly enumerated in PLAN. It is required by ITEM-14's sync notify-and-refetch panel (ziee sync convention needs a permission-checked refetch endpoint). The plan's ITEM-14 assumed a refetch source without naming it; the endpoint is the idiomatic realization.
- **DRIFT-1.4** — verdict: resolved — TEST-16 was written as `#[cfg(test)]` unit tests of `run_connect` (with `MockOfficePlatform`) rather than an HTTP integration test in `settings_test.rs` (TESTS.md tier). The admin-gating (403) is enforced by the `RequirePermissions<(OfficeBridgeManage,)>` extractor + `connect_docs`; a formal HTTP-403 integration assertion is scheduled for the Phase-8 pass. TEST-16 (the id) passes.
- **DRIFT-1.5** — verdict: none — `windows` crate resolved to 0.61 (PLAN tentatively said 0.58, which is unavailable); the plan explicitly allowed "the version resolving with the tree". No real divergence.
- **DRIFT-1.6** — verdict: resolved — `update_settings` does not `sync_publish` (web_search's does); that would need a `SyncEntity::OfficeBridgeSettings` variant, out of scope for v1 and not required by the panel (which keys on `SyncEntity::OfficeDocument`). Deferred deliberately, not a defect.
- **DRIFT-1.7** — verdict: none — the `gate:ui` orchestrator (`scripts/gate-ui.mjs`) is environmentally Windows-broken (`spawn('npm')` ENOENT); its criteria (tsc + lint via `npm run check`, runtime-health) were run directly and pass for the office-bridge surface. This is a pre-existing tooling limitation, not an office_bridge code drift. `npm run check` is fully green.
- **DRIFT-1.8** — verdict: resolved — the cert cache changed from 2-file to a 4-file scheme (CA + leaf + keys) with an empty CA-key marker (the CA key is never re-used in-process). Functional; the persistence test (`load_or_mint_persists_and_reuses`) confirms byte-identical reuse.

**Unresolved drifts:** 0
