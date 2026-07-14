# Chunk `ziee-test-harness` — DRIFT round 1

Drift = any behavioural divergence between the pre-move harness and the
extracted engine + shim, checked symbol-by-symbol against the pre-move blob.

- **DRIFT-1.1** — verdict: none. `SpawnedServer` struct + `Drop` logic (kill →
  wait → rm config → async terminate-backends → `DROP DATABASE`) byte-identical
  to the pre-move `TestServer`; only the admin URL source changed (own field vs
  module fn — same value).
- **DRIFT-1.2** — verdict: none. `ensure_test_template` / `test_template_db`:
  DROP/CREATE/Migrator loop + `set_ignore_missing(true)` + quiesce logic
  unchanged; template name = `template_db_base(variant) + worktree_suffix` = the
  pre-move `if is_desktop() {…} + worktree_suffix()`.
- **DRIFT-1.3** — verdict: none. `make_isolated_data_dir` / `shared_test_app_data_dir`:
  same `prefix("ziee-test-data-")`, same symlink set `[bin,lib,llm-engines,
  lit-cache]`, same repo-root `.parent().parent()` walk — now over the passed
  `manifest_dir` (identical value from the shim).
- **DRIFT-1.4** — verdict: none. `worktree_suffix`: same
  `should_auto_isolate(&DATABASE_URL) ? "_{worktree_key(manifest_dir)}" : ""`.
- **DRIFT-1.5** — verdict: none. Config YAML: diffed the shim's `plan_spawn`
  format string against the pre-move template — every literal line, the
  single-quote-path rationale comment, the sandbox/consent/public_base_url
  branch, and the update_check/bio_mcp/control_mcp/voice sections are identical;
  only interpolated inputs are re-sourced from `SpawnFacts`.
- **DRIFT-1.6** — verdict: move-fix. The temp-config filename prefix changed
  (`ziee-test-` → `testharness-`). Behaviourally inert (ephemeral temp file no
  test reads; deleted on Drop). Kept the change (the SDK must be app-agnostic);
  logged as an intended, inert delta, not a regression.
- **DRIFT-1.7** — verdict: none. Binary-path 3-candidate walk + `.exe` suffix +
  `--headless` argv + `ZIEE_HUB_DATA_DIR_OVERRIDE` env + `opts.extra_env`
  merge: identical, now split across `SpawnPlan` (binary_name/extra_argv/
  extra_env) but assembled from the same inputs in the same order.
- **DRIFT-1.8** — verdict: none. `test_helpers` (register→group RBAC SQL,
  `query!` on `is_default`, the three user-variant fns) copied verbatim into the
  shim; still ziee-schema-coupled, still app-side.

**Unresolved drifts:** 0
