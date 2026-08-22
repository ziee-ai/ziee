# DESIGN_FIDELITY — sandbox honest diagnostics

- **INV-1** — fidelity: UPHELD — ITEM-1/2/3 route every `SANDBOX_NOT_INITIALIZED`
  message through `config::init_status().explain()`, so the error names the ACTUAL
  recorded reason and the false "not yet booted / module disabled" disjunction is
  removed. Pinned by TEST-1 (per-variant `explain()`) + TEST-2 (execute-path error
  carries the reason).
- **INV-2** — fidelity: UPHELD — ITEM-4 splits the write outcome into
  `ChildGone(EPIPE)` (logged `debug`, accurate wording) vs `Truncated` (logged
  `error`, loud hardening-failure wording preserved). Pinned by TEST-3, which
  asserts BOTH branches — the loud case stays loud (the trap the task warns about).
- **INV-3** — fidelity: UPHELD — ITEM-5 redacts the known host roots from captured
  stdout/stderr before the success result is built, extending the no-host-path-leak
  invariant to the `isError:false` path. Pinned by TEST-4 (pure redaction) + TEST-5
  (a simulated bwrap dead-mount stderr is scrubbed in the returned result).
