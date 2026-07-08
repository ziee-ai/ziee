# PLAN — office-bridge pane RPC (ITEM-9 pane path)

Complete the office_bridge feature's remaining gap: the **daemon↔pane JSON-RPC**
over the existing WSS `/bridge` so the 5 pane-mediated tools actually execute
Office.js in the connected task pane instead of returning `OFFICE_PANE_REQUIRED`.

Scope note: the pane path is **cross-platform** — the same `taskpane.js` runs in
WKWebView (macOS) and WebView2 (Windows) against the same backend broker, so no
`platform/{macos,windows}.rs` change is involved. Those files own only the
*native* (osascript/COM) path, which is already done.

## Items

- **ITEM-1**: Bridge broker — `bridge/broker.rs` (new). A process-global registry of
  connected panes (`PaneId → { host, doc_key, tx: mpsc::UnboundedSender<Message> }`)
  plus a pending-request correlation map (`corr_id → oneshot::Sender<BridgeResponse>`)
  and an `AtomicU64` correlation counter. Public API: `register_pane`,
  `unregister_pane`, `route_response` (called by the socket loop), and
  `call_pane(doc_full_name, method, params) -> Result<Value, AppError>` which
  resolves the target pane, allocates a corr id, registers a oneshot, pushes a
  `BridgeRequest` down the pane's `tx`, and awaits the reply with a wall-clock
  timeout — mapping a pane JSON-RPC error / no-pane / timeout to typed `AppError`s.
  Pane resolution: exact `doc_key` match, else the sole connected pane, else a
  typed "no pane connected" error.
- **ITEM-2**: WSS socket loop — rewrite `bridge/server.rs::handle_socket` from an
  echo into a broker-connected duplex: a single task `tokio::select!`s between an
  `mpsc::UnboundedReceiver<Message>` (outbound — so `call_pane` in another task can
  push frames, sent on the socket in the outbound arm) and `socket.recv()` (inbound),
  classifying each inbound frame by JSON-RPC shape — a frame with `method` is a
  pane→daemon request/notification (`register` hello → `broker::register_pane`;
  `ping`/`selection_changed` → debug-log/ack), a frame with `result`/`error` is a
  response to a daemon→pane request → `broker::route_response`; junk is ignored.
  Unregister the pane on close. (DRIFT-1.2: select! duplex chosen over a split-sink +
  writer-task — same effect, no `futures` split dependency.)
- **ITEM-3**: Wire the 5 pane tools — `handlers.rs::dispatch_tool`: replace the
  `pane_required_err(name)` arms for `read_document` / `get_selection` /
  `get_tracked_changes` / `add_comment` / `set_track_changes` with a call to
  `broker::call_pane(doc_full_name, <method>, <params>)`. Keep the existing
  PowerPoint capability pre-gate for `add_comment` / `set_track_changes` (native
  `doc_host` lookup → `unsupported_on_ppt_err`). Map broker outcomes to the typed
  errors: `OFFICE_PANE_NOT_CONNECTED` (open the task pane), `OFFICE_PANE_TIMEOUT`,
  and a propagated pane error (host-unsupported / Office.js failure).
- **ITEM-4**: Task-pane RPC servicing — `resources/office-bridge/taskpane.js`: on WSS
  open send a `register` request carrying `{host, doc_key}` (doc_key from
  `Office.context.document.url`); on each inbound frame that is a daemon→pane request
  (method ∈ the 5 ops), execute it via Office.js and reply with a correlated
  `{jsonrpc, id, result}` or `{jsonrpc, id, error}`. Per-host coverage: `get_selection`
  (host-agnostic `getSelectedDataAsync`), `read_document` (Word body / Excel used
  range), `add_comment` + `set_track_changes` + `get_tracked_changes` (Word); an op on
  a host that doesn't support it replies with a JSON-RPC error the backend surfaces as
  host-unsupported. Preserve the existing selection-change forwarding + ping.
- **ITEM-5**: Windows closeout — a `WINDOWS_PANE_VERIFICATION.md` manual live
  checklist (the analog of the Mac report: WebView2 loads
  `https://localhost:44300/taskpane.html` prompt-free, WSS connects, each of the 5 ops
  round-trips in real Word/Excel) so a Windows+Office operator can close the gap.
  (DRIFT-1.1: a `#[cfg(windows)] #[ignore]` cargo test was planned but dropped — the
  real WebView2 pane cannot be driven from a cargo test, exactly as the Mac
  verification is a manual doc; the cross-platform *backend* is proven by the
  non-cfg-gated `pane_rpc_test.rs`, which also runs on Windows.)
- **ITEM-6**: Mac live verification + report — extend `MAC_OFFICE_BRIDGE_VERIFICATION.md`
  with the live pane-RPC round-trip results (each of the 5 ops driven through a real
  Excel/Word task pane on this box).

## Files to touch

- `src-app/desktop/tauri/src/modules/office_bridge/bridge/broker.rs` (**new**)
- `src-app/desktop/tauri/src/modules/office_bridge/bridge/mod.rs` (declare `broker`)
- `src-app/desktop/tauri/src/modules/office_bridge/bridge/server.rs` (`handle_socket` rewrite)
- `src-app/desktop/tauri/src/modules/office_bridge/handlers.rs` (`dispatch_tool` pane arms + new typed errors)
- `src-app/desktop/tauri/resources/office-bridge/taskpane.js` (RPC servicing)
- `src-app/desktop/tauri/tests/office_bridge/mod.rs` (register the new test module)
- `src-app/desktop/tauri/tests/office_bridge/pane_rpc_test.rs` (**new** — cross-platform mock-pane integration)
- `src-app/desktop/tauri/tests/office_bridge/bridge_test.rs` (update: the removed echo)
- `WINDOWS_PANE_VERIFICATION.md` (**new**, repo root — Windows manual live checklist, DRIFT-1.1)
- `MAC_OFFICE_BRIDGE_VERIFICATION.md` (extend)

## Patterns to follow

- **Broker registry + process-global state** → mirror `bridge/auth.rs`
  (`LazyLock<RwLock<…>>`, poison-recovering `unwrap_or_else(|p| p.into_inner())`,
  bounded store) for the pane/pending maps.
- **Correlation-over-a-channel + oneshot reply** → mirror
  `mcp/elicitation/registry.rs` (a `Lazy<Mutex<HashMap<id, {tx: oneshot::Sender}>>>`
  with `register` / `respond` / `take`, poison-recovering) — the repo's exact
  "register a pending request keyed by id, resolve it from another task via a
  oneshot" idiom (the ask_user/elicitation correlation). The `call_pane` timeout
  wraps the oneshot recv in `tokio::time::timeout`.
- **JSON-RPC envelopes** → reuse the existing `bridge/protocol.rs`
  (`BridgeRequest`/`BridgeResponse`/`BridgeError`) verbatim; no new wire types.
- **Typed tool errors** → mirror the existing `handlers.rs` `pane_required_err` /
  `unsupported_on_ppt_err` constructors (`AppError::new(StatusCode, CODE, msg)`).
- **Mock-pane integration test** → mirror `tests/office_bridge/bridge_test.rs`
  (ephemeral `server::start(0, tempdir)`, a `tokio-tungstenite` client acting as the
  pane) — extend it with a client that answers a daemon→pane request.
- **`#[cfg(windows)] #[ignore]` live test** → mirror `tests/office_bridge/windows_com_test.rs`.
- **Task-pane JS** → mirror the existing `resources/office-bridge/taskpane.js`
  structure (`Office.onReady`, `send`, `log`); keep it dependency-free ES5.
