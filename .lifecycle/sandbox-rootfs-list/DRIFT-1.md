# DRIFT-1 — implementation vs plan

Audited the implementation against PLAN.md item by item after finishing all
items (backend compiles, unit + integration + parity + `npm run check` green).

- **DRIFT-1.1** — verdict: none — ITEM-1..4 (backend) implemented exactly as planned: `SandboxAvailability` enum + `INIT_STATUS` OnceCell in config.rs; reason recorded at all four `init()` early-returns + success; `availability` field on `VersionStatus` + `available_only`; handler returns 200 with the reason. Unit (3) + integration (15, incl. the 2 new/updated) green.
- **DRIFT-1.2** — verdict: resolved — added pure helpers `host_arch()`/`host_package()` and a pure `build_degraded(availability, available)` split from the async `available_only` in version_manager.rs. This is DEC-2 (unit-testability); the plan anticipated it. No plan change needed.
- **DRIFT-1.3** — verdict: resolved — the "disable install in degraded mode" plan item was implemented by extending the shared `RenderButton` with a generic `disabledReason` prop (threaded via `AvailableRootfsCard`→`RootfsVersionGroup`), rather than a bespoke one-off. This is a cleaner realization of the same intent; within ITEM-6.
- **DRIFT-1.4** — verdict: resolved — the store also copies `availability` from the `setPin`/`deleteArtifact` responses (not only `loadStatus`), keeping state coherent after a mutation. Beyond the literal PLAN wording (loadStatus) but within ITEM-5's intent; harmless (those paths only succeed when initialized ⇒ `ready`).
- **DRIFT-1.5** — verdict: none — docker ITEM-7..9 implemented as planned (host deps, `ZIEE_CODE_SANDBOX_ENABLED` plumbing mirroring `ZIEE_UPDATE_CHECK`, minimal-caps overlay + README opt-in section). ITEM-10 regen done for BOTH `ui` and `desktop/ui`; both parity tests green.
- **DRIFT-1.6** — verdict: none — environment-only findings (host had no node/rust/libseccomp/tauri-deps; `src-app/target` is a pre-existing committed broken symlink) were resolved by tooling setup + an out-of-tree `CARGO_TARGET_DIR`; none are code drift and none touch the committed diff. The stray `src-app/target` symlink is restored untouched before commit.

**Unresolved drifts:** 0
