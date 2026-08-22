# INFRA_INTEGRATION — sandbox honest diagnostics

## UX walk (per item)

- **INV-1 (ITEM-1/2/3)**: The human encountering this is an OPERATOR /
  INVESTIGATOR reading a failed run or a server log, not the chat model. The
  improved message ("code_sandbox is not available: the host does not support the
  sandbox (on Linux this means bwrap is not installed or not on PATH)") lands on:
  (a) background-run error records (the 31-failure case — `execute_command_impl`
  returns the `AppError` directly), (b) server logs (the `tracing::warn!(error =
  ?app_err)` at `handlers.rs` dispatch), and (c) the `version_handlers` REST
  responses handed to the admin HTTP client verbatim.
- **INV-2 (ITEM-4)**: The human is an operator watching logs / an alerting
  pipeline. After the fix the routine EPIPE drops to `debug` (off by default) so
  the ~204/day alert-fatigue stream disappears, while a genuine truncation still
  fires `error!` loudly.
- **INV-3 (ITEM-5)**: The human is the chat model (and, transitively, the end
  user reading the model's context). It now sees `<sandbox-rootfs>/usr` instead of
  a real host path on a dead-mount failure — no host filesystem layout leaks.

## Infrastructure-integration walk

- **MCP JSON-RPC error mapping (`map_tool_error`, handlers.rs:249-262)** — On the
  interactive chat path, a 5xx `AppError` is deliberately collapsed to a generic
  envelope so host paths / internals never reach the model. Verified: the richer
  `SANDBOX_NOT_INITIALIZED` message therefore does NOT leak to the model on the
  chat path (correct — the reason is for operators), while the FULL message is
  still logged server-side (`error = ?app_err`) and returned verbatim on the
  non-MCP surfaces (background runs, REST version endpoints). So INV-1's audience
  (operators/investigators) is served without weakening the model-facing posture.
- **Success-result path (`execute_command_with_mounts` → streaming.rs:357 /
  handlers.rs:306)** — This is the gap INV-3 closes: `Ok(value)` is serialized to
  a text content block with `isError:false`, so any host path in captured
  stdout/stderr would reach the model. Redaction is applied inside
  `execute_command_with_mounts`, so BOTH the streaming path and the single-shot
  dispatch path (which both call it) are covered by one fix point.
- **seccomp pipe spawn (`SeccompPipe::install`)** — the write runs in a detached
  `tokio::spawn`; only its LOGGING changed. The write loop, EINTR/EAGAIN retry,
  pipe close, and Drop are untouched, so no lifecycle/ordering behavior changes.
- **Settings / sync / permissions / approval** — none touched. No new tunable
  (DEC-5), no new entity, no new permission.

## Entity-lifecycle walk

No entity, surface, or cached state is added or removed by this branch — the
changes are message text, a log level, and an output-redaction transform. There
is no add/remove/mutate/access-loss surface to exercise.
