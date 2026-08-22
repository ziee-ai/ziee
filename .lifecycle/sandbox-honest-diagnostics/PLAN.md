# PLAN — sandbox honest diagnostics

Fix a cluster of *misleading-diagnostic* defects in `code_sandbox`. These are
about what the sandbox REPORTS (error message text / log level / redaction), not
about execution — the sandbox runs fine. Each was surfaced by a live audit rig
and cost a human investigator real time by asserting a cause the code does not
actually know.

## Design source

No prior design doc exists for these defects — they are bug reports mined from a
live audit rig. Per the lifecycle rule ("if there is genuinely no prior design
doc, WRITE one first and name it"), the governing intent is the **existing, in-
code invariants** each defect violates, lifted verbatim below as the design
source:

- Realizes the **no-host-path-leak invariant** documented at
  `src-app/server/src/modules/code_sandbox/handlers.rs:249-254` (the
  `map_tool_error` doc-comment) — currently enforced ONLY on the JSON-RPC error
  (5xx) path; Defect 3 extends it to the success-result path.
- Realizes the **`SandboxAvailability` contract** at
  `sdk/crates/ziee-sandbox/src/config.rs:9-33` — "every variant is a specific
  early-return reason recorded by `init()`" — the reason exists and must be told
  to the caller (Defect 1).
- Realizes the **seccomp-write SECURITY comment** at
  `sdk/crates/ziee-sandbox/src/sandbox.rs:1021-1029` — a genuine truncated write
  is a real hardening-not-delivered event and must stay loud; a routine
  child-exited-early EPIPE is not and must not (Defect 2).

## Invariants

- **INV-1**: A `SANDBOX_NOT_INITIALIZED` error MUST state the ACTUAL recorded
  reason from `config::init_status()` (e.g. host-unsupported / disabled-in-config
  / workspace-init-failed), NOT a guessed disjunction like "module disabled or
  not yet booted". (`init()` is a set-once `OnceCell`; "not yet booted" can never
  become true later on a live server, so it is a false clause.)
- **INV-2**: A seccomp-pipe write that ends short because the bwrap child closed
  the read end (EPIPE / errno 32) MUST be logged as a routine child-exited-early
  event at a non-alarming level with accurate wording (NOT "hardening claim was
  not delivered / WILL FAIL"). A genuine truncated write (any non-EPIPE cause)
  MUST stay at ERROR with the loud hardening-failure wording. The two cases MUST
  be distinguished.
- **INV-3**: A host absolute path MUST NOT reach the model verbatim in a tool
  RESULT — including a success (`isError:false`) result whose captured
  stdout/stderr carries bwrap's own setup-failure diagnostics (e.g. a dead rootfs
  mount). This extends the existing no-host-path-leak invariant (today enforced
  only on the 5xx error path) to the success path.

## Items

- **ITEM-1**: Add `SandboxAvailability::explain(self) -> &'static str` in
  `sdk/crates/ziee-sandbox/src/config.rs` returning an accurate, operator-facing
  reason per variant.
- **ITEM-2**: Replace the misleading `SANDBOX_NOT_INITIALIZED` message on the
  SDK execute path (`sdk/crates/ziee-sandbox/src/tools/execute.rs:75-81`) with
  one that includes `config::init_status().explain()`.
- **ITEM-3**: Replace the misleading `SANDBOX_NOT_INITIALIZED` messages at the
  ziee server sites (`code_sandbox/handlers.rs` execute path + download path,
  `code_sandbox/version_handlers.rs` ×3, `code_sandbox/tools/files.rs`) so each
  includes `config::init_status().explain()`. (handlers.rs:925 is the site that
  produced the 31 misattributed background-run failures.)
- **ITEM-4**: Introduce a pure classifier for the seccomp-pipe write outcome
  (`Complete | ChildGone(EPIPE) | Truncated`) in
  `sdk/crates/ziee-sandbox/src/sandbox.rs` and log ChildGone at `debug` with
  accurate wording, Truncated at `error` with the existing loud wording.
- **ITEM-5**: Add a pure `redact_host_paths` helper and apply it to the captured
  stdout/stderr in `execute_command_with_mounts`
  (`sdk/crates/ziee-sandbox/src/tools/execute.rs`) so known host roots
  (`state.workspace_root`, the resolved rootfs `mount_dir`) are replaced with
  sandbox-relative placeholders before the result is returned.

## Files to touch

- `sdk/crates/ziee-sandbox/src/config.rs` (ITEM-1)
- `sdk/crates/ziee-sandbox/src/tools/execute.rs` (ITEM-2, ITEM-5)
- `sdk/crates/ziee-sandbox/src/sandbox.rs` (ITEM-4)
- `src-app/server/src/modules/code_sandbox/handlers.rs` (ITEM-3)
- `src-app/server/src/modules/code_sandbox/version_handlers.rs` (ITEM-3)
- `src-app/server/src/modules/code_sandbox/tools/files.rs` (ITEM-3)

## Patterns to follow

- **ITEM-1 / explain()**: mirror the existing `impl SandboxAvailability` +
  `#[serde(rename_all = "snake_case")]` block and its `#[cfg(test)]`
  `availability_serializes_snake_case` test in the SAME file
  (`config.rs`) — add a sibling `explain()` and a sibling per-variant unit test.
- **ITEM-4 / classifier**: mirror the existing pure-logic-extracted-for-testing
  pattern already used in this crate (`pin_or_detect_flavor_switch` in
  `tools/execute.rs`, extracted specifically so the state machine is
  unit-testable) — extract a small pure `fn` + enum and test it in-source.
- **ITEM-5 / redaction**: mirror the crate's existing pure helper +
  `#[cfg(test)]` idiom (`append_capped` / `lossy` in `backend/vm_client.rs`,
  `lossy_string_with_marker` in `sandbox.rs`) — a pure `&str -> String`
  transform with in-source unit tests.
- **ITEM-2 / ITEM-3 messages**: mirror the existing `AppError::new(status,
  "CODE", msg)` construction already at each site; only the message string
  changes.

## Plan audit (phase 2)

Audited against the codebase (`db2347928` + submodule `chat` tip `584756d`).

### Breakage risk

Message strings and log levels are not part of any contract; changing them
breaks no caller. `SandboxAvailability::explain()` is additive (new method).
`redact_host_paths` only rewrites captured output the workload never contains
verbatim (host roots are invisible inside the sandbox), so it is a no-op on all
legitimate output — it only fires on bwrap's own setup diagnostics. The seccomp
classifier is a pure refactor of the existing `offset < total` branch; the loud
ERROR case is preserved byte-for-byte for the genuine-truncation path.

### Pattern conformance

All three helpers mirror in-crate precedent (`pin_or_detect_flavor_switch`,
`availability_serializes_snake_case`, `append_capped`/`lossy`) — pure fn +
in-source `#[cfg(test)]`. Message edits reuse the existing `AppError::new` shape.

### Migration collisions

None — no migration added (see BASE.md).

### OpenAPI regen

Not required — no handler/type/schema change (see BASE.md). `explain()` is a
method, not a serialized field.

### Per-item verdicts

- **ITEM-1** — verdict: PASS — additive method on an existing enum; mirrors the
  sibling `impl`/test in config.rs. No caller impact.
- **ITEM-2** — verdict: PASS — one message string on the SDK execute guard;
  `config` is already in scope (`use crate::config`).
- **ITEM-3** — verdict: PASS — message-only edits at 6 sites; `config` is
  imported at each (`crate::modules::code_sandbox::config`). `crate::common::AppError`
  re-exports `ziee_core::AppError` (verified `src/common/type.rs:17`), the same
  type `explain()` feeds, so a shared reason string works everywhere.
- **ITEM-4** — verdict: PASS — pure classifier extracted from the existing spawn
  task; the genuine-truncation ERROR branch is unchanged. No behavior change to
  the write loop itself.
- **ITEM-5** — verdict: PASS — `state.workspace_root` and `ensure.mount_dir` are
  both already in scope in `execute_command_with_mounts`; redaction is applied to
  `result.stdout`/`result.stderr` immediately before building `response`.
