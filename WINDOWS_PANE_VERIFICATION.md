# Windows Office Bridge — Pane-RPC Verification Checklist

**Status: NOT YET RUN.** The pane-RPC path (ITEM-9) was implemented and verified
live on macOS (see `MAC_OFFICE_BRIDGE_VERIFICATION.md`). The pane path is
**cross-platform** — the same `taskpane.js` (Office.js) runs in WebView2 (Windows)
against the same backend broker (`bridge/broker.rs` + `bridge/server.rs`), so the
*backend* correlation is already proven on Mac by the cross-platform integration
tests (`tests/office_bridge/pane_rpc_test.rs`, TEST-6/7/8/9/12). What remains
Windows-specific is confirming that **WebView2** (not WKWebView) loads the pane and
runs Office.js the same way. This checklist is the analog of the Mac spike; run it on
a real Windows + Microsoft Office box.

## Preconditions
- Windows 10/11 with desktop Microsoft Office (Word/Excel/PowerPoint) installed.
- A `ziee-desktop` build for Windows (`cargo build -p ziee-desktop`), `office_bridge`
  enabled (default), `code_sandbox` off.
- Run the ignored live test as the automated portion:
  ```powershell
  cargo test -p ziee-desktop --test <office_bridge test target> -- --ignored office_bridge::pane_rpc_windows
  ```
  (`pane_rpc_windows_test.rs` is `#[cfg(windows)]`, env-gated on `ZIEE_OFFICE_LIVE`.)

## The Windows-specific unknowns (analogous to the Mac spike's WKWebView unknown)
1. **WebView2 cert trust** — does the WebView2 task pane load
   `https://localhost:44300/taskpane.html` **prompt-free** after `install_cert_trust`
   (`certutil -addstore -f Root` via the elevated `ShellExecuteExW`, one UAC)? (On Mac
   this was the `security add-trusted-cert` → WKWebView gate, which PASSED.)
2. **WebView2 same-origin WSS** — does the pane's `wss://localhost:44300/bridge`
   connect back and select the `ziee-bridge` subprotocol without a mixed-content / CSP
   / cert error?
3. **Office.js op parity** — do the 5 ops execute in Word (and Excel for
   read/selection) via the same `Word.run`/`Excel.run` calls that worked on Mac?

## Round-trip checklist (record PASS / FAIL + exact error for each)

- [ ] **Boot**: server log shows `office_bridge: bridge listening on https://localhost:44300`.
- [ ] **Connect**: `POST /api/office-bridge/connect` (or the settings [Connect] button)
      trusts the cert (one UAC) and sideloads the manifest into the HKCU WEF
      `Developer` key.
- [ ] **Task pane loads**: in Word, Home ribbon → "Ziee" group → "Show Ziee Bridge"
      opens the pane and it loads **without a cert warning** (unknown 1). Pane log shows
      `Office.onReady host=Word` + `bridge open (…, token=present)` + `registered (…)`.
The office surface is now TWO tools: `list_open_documents` (native discovery) and
`run_office_js` (everything else — reads, comments, track-changes are all Office.js
scripts now; the former typed tools are removed). `run_office_js` declares
`mode:"read"|"write"`; a `read` auto-runs, a `write` prompts for approval.

- [ ] **list_open_documents**: returns the open docs (name/path/host/saved).
- [ ] **run_office_js — read (auto-run)**: agent calls `run_office_js` with
      `mode:"read"` and `const r = context.workbook.worksheets.getActiveWorksheet().getRange('A1'); r.load('values'); await context.sync(); return r.values;` → runs **without an approval prompt** and returns A1's value.
- [ ] **run_office_js — write (approval)**: agent calls `run_office_js` with
      `mode:"write"` and `const r = context.workbook.worksheets.getActiveWorksheet().getRange('A1'); r.values = [['ziee-run']]; r.load('address'); await context.sync(); return r.address;` → the chat surfaces an **approval prompt (allow once / always allow / deny)**; on approve, A1 shows `ziee-run` and `structuredContent.result` is A1's address.
- [ ] **run_office_js — Word comment / track changes (write)**: a script that
      `insertComment`s on a `body.search(...)` hit, or sets `context.document.changeTrackingMode`, lands the change (both `mode:"write"`).
- [ ] **run_office_js — structured error**: a deliberately-broken script (bad range)
      returns a STRUCTURED `OFFICE_PANE_ERROR` (name/message/Office.js code), not a crash.
- [ ] **Always-allow persists**: after choosing "always allow" for a write, subsequent
      `mode:"write"` calls in the same conversation run without a prompt.
- [ ] **No-pane error**: with the pane closed, `run_office_js` returns
      `OFFICE_PANE_NOT_CONNECTED` (open the pane and retry).

## If everything passes
Record the results here (mirroring the Mac report), and the Windows pane path is
verified. No code change should be needed — the backend is shared and already proven;
this only confirms the WebView2 host behaves like WKWebView. If an op fails, the fix is
almost certainly in `resources/office-bridge/taskpane.js` (the Office.js call), which is
shared, so a fix there also benefits Mac — re-run the Mac checklist after any change.
