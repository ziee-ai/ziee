# DRIFT-1 — implementation vs plan (office-bridge pane RPC)

Audit of the phase-5 implementation against PLAN.md / TESTS.md. Each divergence is
resolved `plan-wins` (re-implement) or `impl-wins` (amend the plan, with rationale).

- **DRIFT-1.1** — verdict: impl-wins — ITEM-5 planned a `#[cfg(windows)] #[ignore]`
  Rust live test `pane_rpc_windows_test.rs`. Implementation reality: the real
  WebView2 pane path cannot be driven from a `cargo test` — the test process cannot
  reach the *running desktop app's* in-process broker, and no cargo test can open /
  drive a live Office task pane. This is EXACTLY why the Mac verification (TEST-13) is
  a manual doc, not a test. So the Windows closeout is the manual checklist
  `WINDOWS_PANE_VERIFICATION.md` (mirroring TEST-13), and the Rust file is dropped.
  The cross-platform *backend* (broker + socket loop + dispatch) is already proven on
  Mac by `pane_rpc_test.rs` (TEST-6/7/8/9/12), which is not cfg-gated and runs on
  Windows too. PLAN ITEM-5 + Files-to-touch and TESTS TEST-14 amended accordingly.

- **DRIFT-1.2** — verdict: impl-wins — ITEM-2 planned to "split the socket into
  sink+stream, spawn a writer task fed by an mpsc". Implementation uses a single task
  that `tokio::select!`s between the mpsc receiver (outbound) and `socket.recv()`
  (inbound), sending on the same socket in the outbound arm. Same effect (the broker
  still pushes down an mpsc), fewer moving parts, and it avoids a `futures` split-sink
  dependency the audit flagged as unconfirmed. PLAN ITEM-2 wording amended to describe
  the select! duplex.

- **DRIFT-1.3** — verdict: resolved — three test-compile fixes during implementation
  (integration `Message` type disambiguation for decoy channels → `axum::…::Message`;
  `run_mock_pane` `doc_key` made owned for `tokio::spawn`'s `'static`; the removed-echo
  left `SinkExt`/`Message` imports unused in `bridge_test.rs`) and one racy assertion
  removed (`call_pane_times_out` asserted the process-global `PENDING` map's exact size,
  which flakes under concurrent tests — the timeout behavior itself is asserted). No
  plan change; these are implementation details.

**Unresolved drifts:** 0
