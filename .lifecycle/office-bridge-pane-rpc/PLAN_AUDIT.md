# PLAN_AUDIT — office-bridge pane RPC

Audit of PLAN.md against the actual codebase before writing code.

## Breakage risk

- **The `/bridge` echo is removed (ITEM-2), which breaks an existing test.**
  `tests/office_bridge/bridge_test.rs` asserts "(b) `wss://…/bridge` with that token
  + an allowed Origin **echoes a frame**". Replacing `handle_socket`'s echo with the
  broker duplex means a raw non-JSON-RPC frame is no longer echoed. **Resolution:**
  update that test to exercise the new protocol — a mock pane connects, sends
  `register`, and answers a daemon→pane request — rather than a bare echo. Covered by
  the ITEM-2 test rewrite (TESTS.md).
- **Two in-source tests assert `OFFICE_PANE_REQUIRED` (ITEM-3 changes their result).**
  `handlers.rs::test12_pane_mediated_method_returns_pane_required_error` and
  `test12_add_comment_on_word_returns_pane_required_error` assert the 5 tools return
  `OFFICE_PANE_REQUIRED`. Once wired to the broker with **no pane connected**, they now
  return `OFFICE_PANE_NOT_CONNECTED`. **Resolution:** update both tests to assert the
  new no-pane code, and add a mock-pane path proving the happy case. The `OFFICE_PANE_REQUIRED`
  constant/`pane_required_err` is removed (no longer reachable) — confirm no other
  reference (grep shows only these two tests + the fn/const).
- **PPT capability pre-gate is preserved (no breakage).** `add_comment`/`set_track_changes`
  still call native `doc_host` and return `unsupported_on_ppt_err` before touching the
  broker, so `test12`'s PPT-unsupported assertions (integration `mcp_test`) stay valid.
- **No public API / route / DTO change.** The MCP tool schemas (`tools.rs`) and the
  REST surface are unchanged; the broker is purely internal + the WSS wire uses the
  already-present `protocol.rs` envelopes. No caller outside office_bridge is affected.
- **Concurrency correctness is the main risk, not breakage.** The broker introduces a
  cross-task push (writer mpsc) + pending-oneshot map; a dropped socket must fail
  in-flight `call_pane`s (oneshot sender dropped → recv errors → typed error) rather
  than hang. Explicitly covered by the timeout + a drop test (TESTS.md ITEM-1/2).

## Pattern conformance

- **Broker registries** conform to `bridge/auth.rs` (`Lazy/LazyLock` + `Mutex/RwLock`,
  poison-recovering `unwrap_or_else(|p| p.into_inner())`, bounded where a map could
  grow). The pane map is naturally bounded by live sockets; the pending map entries are
  removed on resolve/timeout.
- **Pending-correlation** conforms to `mcp/elicitation/registry.rs` — the verified
  in-repo twin (`Lazy<Mutex<HashMap<id, {tx: oneshot::Sender}>>>` + `register`/`respond`/
  `take`). `call_pane` adds `tokio::time::timeout` around the oneshot recv.
- **WSS split-sink + writer-task** — axum `WebSocket` splits via `futures::StreamExt`;
  the mpsc-fed writer task is the standard idiom. Confirm `futures` is available to the
  crate (it is, transitively via axum; add a direct dep only if the split helpers need
  it — flagged for the impl phase).
- **Mock-pane test transport** — `tokio-tungstenite 0.27` is already a `desktop/tauri`
  dev-dep and `bridge_test.rs` already uses it, so the mock pane client mirrors an
  existing test.
- **taskpane.js** stays dependency-free ES5 mirroring the current file.

## Migration collisions

None. This feature adds **no migration** — the broker is in-memory process state and
no schema/settings column changes. (Latest desktop migration is
`10000000000007_grant_office_bridge_permissions_to_users.sql`; untouched.)

## OpenAPI regen

Not required. No DTO, permission, sync-entity, or route change. The new typed error
codes (`OFFICE_PANE_NOT_CONNECTED`, `OFFICE_PANE_TIMEOUT`) are runtime JSON-RPC error
strings inside the MCP `tools/call` payload, not OpenAPI schema types — the same class
as the existing `OFFICE_PANE_REQUIRED`/`OFFICE_UNSUPPORTED_ON_HOST`, which are not in
`openapi.json`. `just openapi-regen` is a no-op here; `types_ts_parity` stays green.

## Per-item verdicts

- **ITEM-1** — verdict: PASS — new self-contained `broker.rs`; mirrors `auth.rs` +
  `elicitation/registry.rs`; no external caller; timeout/drop semantics specified.
- **ITEM-2** — verdict: CONCERN — `handle_socket` rewrite removes the echo, breaking
  `bridge_test.rs`'s echo assertion; resolved by rewriting that test to the new
  protocol (a mock pane). No production caller depends on echo (it was the placeholder).
- **ITEM-3** — verdict: CONCERN — rewiring the 5 pane arms changes the no-pane result
  from `OFFICE_PANE_REQUIRED` to `OFFICE_PANE_NOT_CONNECTED`, breaking two in-source
  tests; resolved by updating those tests + removing the dead `pane_required_err`.
  PPT pre-gate preserved.
- **ITEM-4** — verdict: PASS — `taskpane.js` extension is additive (adds an inbound
  request handler + a `register` frame); mirrors the existing file; ES5, no deps. Not
  auto-testable (Office.js in a real host) → verified live on Mac (ITEM-6) + documented
  for Windows (ITEM-5).
- **ITEM-5** — verdict: PASS — a `WINDOWS_PANE_VERIFICATION.md` manual live checklist
  (mirroring the Mac report); the WebView2 pane can't be driven from a cargo test
  (DRIFT-1.1), and the cross-platform backend is proven by `pane_rpc_test.rs`.
- **ITEM-6** — verdict: PASS — documentation extension of the existing Mac report;
  backed by a real live run on this box.

No `BLOCKED` verdicts. The two `CONCERN`s are test updates already budgeted in TESTS.md,
not plan defects.
