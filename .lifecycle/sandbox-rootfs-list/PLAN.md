# PLAN — sandbox-rootfs-list

Fix the empty Settings → Sandbox rootfs list. The admin list endpoint hard-gates
its whole response behind `code_sandbox` being initialized (503
`SANDBOX_NOT_INITIALIZED`), but the `available` catalog comes from GitHub Releases
and needs neither DB nor init. Make the list degrade gracefully (200 + a
machine-readable reason), and make code_sandbox an opt-in in the web container.

## Items

- **ITEM-1**: Add a `SandboxAvailability` enum (`#[serde(rename_all="snake_case")]`, `Serialize`+`JsonSchema`) and a parallel `static INIT_STATUS: OnceCell<SandboxAvailability>` with `set_init_status`/`init_status` (default `NotInitialized`) in `code_sandbox/config.rs`.
- **ITEM-2**: Record the init reason via `config::set_init_status(...)` at each `init()` early-return (disabled-in-config, host-unsupported, imds-refused, workspace-init-failed) and `Ready` at the success tail in `code_sandbox/mod.rs`, leaving every existing `tracing` line unchanged.
- **ITEM-3**: Add `pub availability: SandboxAvailability` to `VersionStatus` (set `Ready` in `status()`), plus a pure `build_degraded(availability, available)` constructor and `pub async fn available_only(availability)` wrapper (uses `list_releases()`, empty on failure) in `version_manager.rs`.
- **ITEM-4**: Rewrite `get_versions_handler` to return **200** always for the LIST path — full `status(pool)` when initialized, else `available_only(reason)` — keeping the `RequirePermissions` auth gate and leaving `install`/`set_pin`/`delete` on their `live_pool()` 503.
- **ITEM-5**: Frontend store `SandboxRootfsVersions.store.ts` carries `availability` (default `'ready'`), derives `isDegraded`, and treats a 200-degraded response as NOT an error (`error` stays reserved for genuine load failures).
- **ITEM-6**: Frontend `SandboxRootfsVersionsSection.tsx` renders an `Alert tone="warning"` (`sandbox-rootfs-degraded-alert`) with reason-specific copy + the `AvailableRootfsCard` when degraded, disables install affordances in degraded mode, keeps `ErrorState` for real failures, and adds a `seeded-sandbox-rootfs-disabled` gallery surface for the new state.
- **ITEM-7**: Install `bubblewrap squashfuse fuse3 fuse` in the `docker/web/Dockerfile` runtime stage.
- **ITEM-8**: Wire `ZIEE_CODE_SANDBOX_ENABLED` (default `false`) through `config.template.yaml`, the `entrypoint.sh` envsubst whitelist, and the Dockerfile ENV block, mirroring `ZIEE_UPDATE_CHECK`.
- **ITEM-9**: Add `docker-compose.sandbox.yaml` (merge overlay, minimal caps: `/dev/fuse` + `SYS_ADMIN` + unconfined apparmor/seccomp, `ZIEE_CODE_SANDBOX_ENABLED: "true"`) and document the opt-in + `privileged` fallback in `docker/web/README.md`.
- **ITEM-10**: Regenerate `openapi.json` + `api-client/types.ts` (both workspaces) so the new enum/field flow through, keeping the `emit_ts` golden parity test green.

## Files to touch

- `src-app/server/src/modules/code_sandbox/config.rs`
- `src-app/server/src/modules/code_sandbox/mod.rs`
- `src-app/server/src/modules/code_sandbox/version_manager.rs`
- `src-app/server/src/modules/code_sandbox/version_handlers.rs`
- `src-app/server/src/modules/code_sandbox/tests` via `src-app/server/tests/code_sandbox/tier3_versions.rs`
- `src-app/ui/src/modules/code-sandbox/stores/SandboxRootfsVersions.store.ts`
- `src-app/ui/src/modules/code-sandbox/components/SandboxRootfsVersionsSection.tsx`
- `src-app/ui/src/modules/code-sandbox/components/AvailableRootfsCard.tsx`
- `src-app/ui/src/dev/gallery/seededSurfaces.tsx`
- `src-app/ui/tests/e2e/settings/sandbox-rootfs-versions.spec.ts`
- `src-app/server/openapi/openapi.json`, `src-app/ui/src/api-client/types.ts`, `src-app/desktop/ui/src/api-client/types.ts` (generated)
- `docker/web/Dockerfile`, `docker/web/config.template.yaml`, `docker/web/entrypoint.sh`, `docker/web/README.md`
- `docker-compose.sandbox.yaml` (new)

## Patterns to follow

- **Module state / enum**: mirror the existing `OnceCell<Arc<CodeSandboxState>>` in `code_sandbox/config.rs`; enum serialization mirrors `version_install_tasks.rs:111` and `runtime_fetch.rs:41` (`rename_all = "snake_case"`).
- **Status handler**: mirror `version_manager::status()` — one 200 call powering the admin page.
- **Frontend notice vs error**: `Alert tone="warning"` for graceful degrade (mirrors `sandbox-rootfs-noperm-alert` and the `SandboxResourceLimitsSection` read-only notice); `ErrorState` reserved for genuine load failure (existing convention in the same file).
- **Env→YAML bool**: mirror `ZIEE_UPDATE_CHECK` (template, envsubst whitelist, Dockerfile ENV).
- **Compose overlay**: alongside `docker-compose.external-db.yml`; keep base `name: ziee-web`.
