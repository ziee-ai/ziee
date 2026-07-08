# PLAN_AUDIT — sandbox-rootfs-list

Audit of the plan against the current codebase (read firsthand:
`version_handlers.rs`, `version_manager.rs`, `mod.rs`, `config.rs`, `probes.rs`,
`SandboxRootfsVersionsSection.tsx`, `SandboxRootfsVersions.store.ts`,
`docker/web/*`).

## Breakage risk

- The only public LIST caller is the frontend store (`ApiClient.CodeSandbox.listRootfsVersions`) and `tests/code_sandbox/tier3_versions.rs`. Changing 503→200 for the LIST path is backward-compatible for the store (it already parses a `VersionStatus`); the one integration test that tolerated `200|503` (`list_versions_passes_permission_gate_for_reader`) is updated in TESTS.md.
- `install`/`set_pin`/`delete` keep `live_pool()` → their 503 behavior and the `tier3_versions` assertions on them are unchanged. `tier3_http.rs` MCP-path 503 assertions are untouched (different route).
- Adding a **new required field** `availability` to `VersionStatus` is additive on the wire; existing consumers ignore unknown/extra fields. The store must read it, but a missing field would only default — handled in FE with a default `'ready'`.
- `SandboxAvailability` is a fresh type; no name collision (`grep` shows no existing `SandboxAvailability`/`INIT_STATUS` in the module).
- Docker: `ZIEE_CODE_SANDBOX_ENABLED` defaults `false`, so the general image and the running `docker-compose.yml` are behavior-identical unless the overlay is used. The envsubst whitelist edit is additive.

## Pattern conformance

- Enum `#[serde(rename_all="snake_case")]` matches `version_install_tasks.rs:111` / `runtime_fetch.rs:41`. `INIT_STATUS` mirrors the existing `OnceCell` in the same `config.rs`.
- Handler returns one 200 `VersionStatus`, mirroring `status()` usage. Auth gate unchanged (`RequirePermissions<(CodeSandboxEnvironmentsRead,)>`).
- FE degrade uses `Alert tone="warning"` — the established non-fatal notice (mirrors `sandbox-rootfs-noperm-alert`, resource-limits read-only notice); `ErrorState` stays for real failures. Consistent with the existing `error`/`sseError` split in the store.
- Env→YAML bool mirrors `ZIEE_UPDATE_CHECK`. Overlay mirrors `docker-compose.external-db.yml`, keeping base `name`.

## Migration collisions

- **None.** No SQL migration is added or touched. `ls src-app/server/migrations/` is not modified; the change is code + config only. The DB schema (`code_sandbox_rootfs_artifacts`, `code_sandbox_settings`) is unchanged; degraded mode simply does not read it.

## OpenAPI regen

- The new `availability` field + `SandboxAvailability` enum change `VersionStatus`'s schema → requires `just openapi-regen` (writes `openapi.json` + `types.ts` in both `ui` and `desktop/ui`). The `emit_ts` golden parity test (`openapi::emit_ts::tests::types_ts_parity`) enforces the regen was run. TS union `'ready' | 'disabled_in_config' | ...` derives from the snake_case enum. Generated files are excluded from the audit-coverage law and do not reclassify the diff as UI-only.

## Per-item verdicts

- **ITEM-1** — verdict: PASS — mirrors the existing `OnceCell` state holder + snake_case enum convention; no collision.
- **ITEM-2** — verdict: PASS — four early-return sites + success tail are all confirmed present in `mod.rs::init` (:194/:213/:233/:244/:332); only adds one call each, keeps logs.
- **ITEM-3** — verdict: CONCERN — `available_only` touches the network via `list_releases()`; split out a pure `build_degraded` so the unit test needs no network. Resolved in DEC-2.
- **ITEM-4** — verdict: PASS — `status(pool)` already tolerates `get_state()==None`; handler branches on state/pool. Auth gate preserved.
- **ITEM-5** — verdict: PASS — additive store field; degrade path avoids setting `error` (matches existing `error`/`sseError` discipline).
- **ITEM-6** — verdict: PASS — reuses kit `Alert`; gallery surface mirrors `seeded-sandbox-limits-error`. New render state ⇒ gallery cell required (state-matrix) — budgeted.
- **ITEM-7** — verdict: PASS — additive apt packages; harmless when sandbox off.
- **ITEM-8** — verdict: PASS — mirrors `ZIEE_UPDATE_CHECK` exactly across the three files.
- **ITEM-9** — verdict: CONCERN — real sandboxed exec may need host unprivileged-userns / privileged; overlay grants minimal caps + README documents the privileged fallback and that exec may be blocked in this environment. Resolved in DEC-3.
- **ITEM-10** — verdict: PASS — mechanical regen; golden parity test is the backstop.
- **ITEM-11** — verdict: PASS — pure `existing ?? initial` guard in a separate testable file; does not touch the SSE handlers; keeps SSE authoritative once a task is tracked. Confirmed the bug via a live SSE repro (server/nginx deliver progress correctly; the POST reply clobbered it). No new API/migration surface.
- **ITEM-12** — verdict: PASS — narrow allowlist keyed on BOTH the id AND the exact sanctioned file set, so a stray dup of those ids elsewhere still fails; touches only the build-infra plugin (main-owned), not the ask-user components or their tests; `npm run build` now passes (1425 unique ids).
