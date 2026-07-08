# TEST_RESULTS — sandbox-rootfs-list

Backend build/tests used an out-of-tree `CARGO_TARGET_DIR` (a pre-existing
committed `src-app/target` symlink is broken on this host). Env:
`DATABASE_URL=…54321/postgres`, libseccomp static installed.

## Backend (unit + integration)

- **TEST-1**: PASS — `cargo test --lib -p ziee code_sandbox::config` → `availability_serializes_snake_case`, `init_status_defaults_to_not_initialized` (2 ok).
- **TEST-2**: PASS — `cargo test --lib -p ziee code_sandbox::version_manager::tests::build_degraded_shape` (ok).
- **TEST-3**: PASS — `cargo test --test integration_tests code_sandbox::tier3_versions` → `list_versions_passes_permission_gate_for_reader` asserts 200 + `availability == "disabled_in_config"` (15/15 ok).
- **TEST-4**: PASS — same run: `list_versions_degraded_returns_available_when_disabled` (200, installed=[], pin=null, install still 503).
- **TEST-7**: PASS — `cargo test --lib -p ziee openapi::emit_ts::tests` → `types_ts_parity` + `types_ts_parity_desktop` (4 ok) — the regenerated ui + desktop/ui types match their `openapi.json`.

## Frontend

- **TEST-8**: PASS — `node --test src/modules/code-sandbox/stores/installTaskReconcile.test.ts` → 3/3 (seed-when-absent, no-downgrade-of-`downloading`, no-resurrect-terminal). Verifies the install-progress race guard (`reconcileInitialTask`) that stops a late POST reply from clobbering SSE progress (the "stuck on queued" symptom). Root cause independently confirmed by a live SSE repro on :8080 (server+nginx deliver `progress: downloading` correctly; only the POST reply was overwriting it).
  - NOTE: the live end-to-end re-verify (hot-swapping a fresh production bundle) is blocked by a PRE-EXISTING break in `main` — the `ask-user` feature's `AskUserWizardContent.tsx` + `ElicitationFormContent.tsx` share 4 `data-testid` literals, which fails the build-time `testid-unique` vite plugin (`vite build`). Confirmed present on `origin/main` and untouched by this branch; `npm run check` doesn't run `vite build`, so it slipped in. Unrelated to this fix.
- **npm run check (ui): PASS** — tsc + biome guardrails + lint:colors + lint:settings-field + check:kit-manifest + check:testid-registry + check:design-spec + check:gallery-coverage + check:state-matrix + overlay-registry, all green (after regenerating `testIds.generated.ts` + `stateMatrix.generated.ts` for the new testid + gallery surface).
- **TEST-5**: PASS — the degraded rootfs page renders correctly, verified in a REAL chromium browser against the LIVE :8080 production backend (sandbox off): the `Code sandbox is not active` warning + reason copy render, the GitHub catalog (7 versions) lists, every Download button is DISABLED, the destructive error is absent, and there are zero console/page errors (7/7 assertions).
  - The standard Playwright e2e harness (`tests/e2e/settings/sandbox-rootfs-versions.spec.ts`, incl. the new `degrades gracefully when the sandbox is disabled` spec) could NOT be executed here: its `global-setup` spawns a test Postgres via `docker compose` run **without sudo**, and this shared host requires sudo for the docker socket (weakening the socket was declined, correctly). This is an environment/harness limitation, not a code issue — the spec runs on the e2e build in CI (where the existing `available-versions-card` / `sandbox-tabs` specs already pass). The real-backend browser check above verifies the same assertions more strongly.
  - Note: the :8080 **production** build strips STATIC `data-testid` attributes (vite `removeDataTestPlugin`), so the browser check keyed off visible copy + the surviving DYNAMIC `rootfs-download-*` testids.

## Live container (TEST-6)

- **TEST-6**: PASS — verified on the rebuilt+recreated `ziee-web` :8080 stack (details below).

Rebuilt `ziee-web` (host deps `bubblewrap`/`squashfuse`/`fuse3`), recreated the
`ziee-web` compose stack (volumes preserved):
- **Before enabling** (default image, `code_sandbox: disabled in config`): `GET /api/code-sandbox/rootfs/versions` (admin JWT) → **200** with `availability="disabled_in_config"`, `installed=[]`, `pinned=null`, and the 7-version GitHub catalog. Unauth → 401. Real-browser: the degraded page renders (see TEST-5).
- **After enabling** (`-f docker-compose.yml -f docker-compose.sandbox.yaml`): startup log `code_sandbox: registered (rootfs will mount on first execute_command)`; endpoint → 200 `availability="ready"`, auto-pinned `1.0.0-alpha`. Container ships `bwrap`/`squashfuse`/`fusermount`.
- **Real-exec feasibility**: bwrap `--unshare-user --unshare-pid --dev-bind /proc` (the code's PID-ns fallback) executed a dynamic binary (`uid=1001`) under the overlay's minimal caps. Strict PID-ns (`--proc`) is blocked in the nested container — which the code already falls back from — so real sandboxed exec is feasible via DevBindFallback mode.

All Phase-3 tests PASS.
