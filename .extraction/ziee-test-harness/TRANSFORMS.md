# Chunk `ziee-test-harness` — TRANSFORMS

Every symbol whose SDK form differs from its pre-move ziee form, and why. The
SQL query text, the config-YAML template, `test_helpers`, and the health-poll /
Drop / template-clone LOGIC are byte-for-byte preserved; the transforms are the
mechanical de-coupling (env! → param, compile-time switch → runtime enum, app
hook extraction) that the seam requires.

- **T-1** `TestServer` struct + `Drop` → **`SpawnedServer`** (lib.rs): renamed;
  the four app-specific lifetime fields (`_workspace_tempdir`,
  `_sandbox_cache_tempdir`, `_hub_tempdir`) collapse into one generic
  `_keep_alive: Vec<Box<dyn Any + Send + Sync>>` the app fills via
  `SpawnPlan.keep_alive`; `Drop` gains an `admin_db_url` field instead of calling
  the module-level `database_url()` (so the SDK holds its own admin URL).
  **why:** orphan rule — the shim needs `impl TestServer { start() }`, which is
  impossible on a foreign type, so the SDK handle is renamed and the shim wraps
  it. The tempdir collapse is what lets the app own ALL app-specific tempdirs
  (workspace/hub/sandbox-cache) generically while the engine still reaps them.

- **T-2** `is_desktop()` → **`Variant` enum** (lib.rs) + `variant()` in the shim.
  **why:** a compiled SDK crate can't read the CONSUMER's `CARGO_PKG_NAME`; the
  shim (still `#[path]`-compiled per crate) reads it and seeds the runtime
  `Variant`, which the engine keys `template_db_base` + `migration_dirs` off —
  preserving the exact server-vs-desktop template split (the desktop template's
  extra migrations).

- **T-3** `worktree_suffix()`, `shared_test_app_data_dir()`,
  `make_isolated_data_dir()`, `test_template_db()`, `ensure_test_template()`,
  the binary-path walk: `env!("CARGO_MANIFEST_DIR")` → a `manifest_dir: &Path`
  param threaded from `TestHarness::new`. **why:** the design-critical
  foot-gun (G2) — inside the SDK crate `env!` resolves to the SDK crate; the
  consumer's value must flow in at runtime. The repo-root walk (`.parent().
  parent()`) + the worktree-key + migration-root joins are byte-identical given
  the same input path (which the shim supplies unchanged).

- **T-4** config generation, workspace tempdir, hub tempdir, binary selection,
  argv/env assembly → moved WHOLESALE into the shim's `HarnessApp::plan_spawn`.
  **why:** the config YAML content (jwt issuer `"ziee"` / audience `"ziee-api"`,
  the `code_sandbox`/`bio_mcp`/`control_mcp`/`voice`/`update_check` sections) +
  the `ZIEE_HUB_DATA_DIR_OVERRIDE` env + the `--headless` flag are all
  app-specific. The format string, the single-quote-YAML rationale, the
  sandbox/consent logic, and every option-driven line are copied verbatim; only
  the interpolated inputs now come from `SpawnFacts` instead of local vars.

- **T-5** `ziee::init_storage_key(...)` + `#[cfg(windows)]
  ensure_sandbox_helper_for_tests()` → `HarnessApp::before_spawn`. **why:** the
  only two `ziee::`-touching pre-spawn side effects; extracted to a default-noop
  hook the app implements. `ensure_sandbox_helper_for_tests` (which calls
  `ziee::sandbox_helper_is_running()`) stays verbatim in the shim.

- **T-6** the temp-config filename `ziee-test-{id}.yaml` → generic
  `testharness-{id}.yaml` (engine-owned). **why:** the config path is an
  ephemeral temp file NO test reads (deleted on Drop); a neutral prefix keeps
  the SDK app-agnostic. Behaviourally inert.

- **T-7** health-poll URL `{base}/api/health` → `{base}{app.health_path()}` with
  `health_path()` defaulting to `"/api/health"`. **why:** tunable seam with
  today's value as the default → byte-identical behaviour for ziee.

- **T-8** the let-chain `if let Ok(r) = … && r.status()…` → nested `if let` +
  `if`. **why:** the SDK workspace is edition 2021 (no let-chains); the server
  crate is edition 2024. Semantically identical.

## Decision — combine `render_config`/`extra_argv`/`before_spawn` into one `plan_spawn`

The audit sketch listed separate `render_config -> String`, `extra_argv`,
`before_spawn` methods. **Resolution:** the app must create per-test tempdirs
(sandbox workspace, hub-override) whose PATH is interpolated into the config /
env AND whose HANDLE must outlive the spawned server. Separate methods cannot
express that path↔lifetime coupling (a `-> String` can't also hand back a
`TempDir`). `plan_spawn(&opts, &SpawnFacts) -> SpawnPlan { config_yaml,
binary_name, extra_argv, extra_env, keep_alive }` bundles them into one call
where the tempdir is created once and both its path and handle are routed. This
is a strict superset of the sketch (it still expresses config + argv + env) and
is the minimal seam that keeps the move equivalence-preserving. `before_spawn`
stays a separate hook (it is a process-global side effect, not per-spawn data).

## Decision — `HarnessApp` is a SYNCHRONOUS trait (no `async_trait`)

**Resolution:** every app coupling today (storage-key init, Windows helper,
string formatting, tempdir creation) is synchronous — the original called them
synchronously (not awaited). Keeping the seam sync avoids an `async-trait` dep
and matches the original control flow exactly. The engine (`TestHarness::start`)
remains `async`; only the seam methods are sync.
