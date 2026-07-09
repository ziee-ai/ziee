# macOS Office Bridge — Verification Report

**Branch:** `feat/office-bridge`  **Host:** Apple Silicon Mac, real Aqua GUI, Microsoft Office (Word/Excel/PowerPoint) installed
**Date:** 2026-07-08  **Verifier:** automated hands-on spike (Claude)

This is the DEC-9 Mac spike that gates `MAC_TRANSPORT_VERIFIED`. Every claim below
was produced by running the **exact** commands/scripts `macos.rs` generates against
live Office + the live macOS trust store, not by reading code.

---

## TL;DR

| Unknown | Verdict |
|---|---|
| (1) `security add-trusted-cert` → WKWebView trusts `https://localhost` prompt-free | **WORKS — confirmed in-app: Office WKWebView task pane loaded prompt-free** |
| (2) osascript enumerates + drives Excel/Word | **WORKS — after fixing 2 real bugs found in this spike** |
| (3) full round-trip: sideload → task pane → WSS connect-back | **WORKS for the transport + osascript tools; the 5 pane-mediated Office.js tools are NOT IMPLEMENTED on this branch (ITEM-9, any platform)** |

**Can `MAC_TRANSPORT_VERIFIED` flip to true?** **YES — and this spike flipped it.**
The literal DEC-9 gate (Keychain cert-trust + WKWebView same-origin-WSS round-trip)
ran and passed on real hardware. The const is a pure gate/doc value — nothing
functional branches on it — so flipping it only required updating its two test
assertions + doc comments (done, committed).

**Definitive in-app evidence** — with the bridge live on 44300, its CA trusted, and
the add-in sideloaded, opening the "Show Ziee Bridge" task pane in **Excel** produced:
```
Office.onReady host=Excel platform=Mac
addHandler status=succeeded
bridge open (host=Excel, token=present)
bridge <- {"jsonrpc":"2.0","id":1,"method":"ping","params":{"host":"Excel","platform":"Mac",...}}
```
The WKWebView loaded `https://localhost:44300/taskpane.html` with **no cert warning
and no trust/password prompt**, opened the same-origin `wss://localhost:44300/bridge`
with its injected token, and round-tripped a ping through the echo. That is unknowns
(1) and (3)-transport proven together, live, through the real Office WKWebView.

---

## Setup deltas from the brief

The brief's paths did not match this machine:
- No `~/ziee`; the repo lives at `/Volumes/zData/Projects/ziee/ziee`. Worktree
  `~/office-check-wt` was absent — recreated from `origin/feat/office-bridge`.
- No `~/node-lts`; used Homebrew node (`/opt/homebrew/bin/node`, v24).
- pgvector submodule initialized; `npm install` at repo root; build DB on `:54321` reachable.

## Architecture reality (important framing for unknown 3)

The `office_bridge` on this branch is **desktop-only** (moved into the `ziee-desktop`
tauri crate per the `office-bridge-desktop-only` re-architecture). Of the 7 office tools:
- `list_open_documents` and `edit_document`(append_paragraph, Word) are served **now**
  via the osascript platform (`platform/macos.rs`).
- The other 5 are **pane-mediated** (Office.js over the WSS bridge) and are
  deliberately stubbed: `dispatch_tool` returns a typed `OFFICE_PANE_REQUIRED`
  capability error. The daemon↔pane RPC is a FUTURE item (ITEM-9).
- `/bridge` WSS is an **echo transport skeleton**, not a JSON-RPC dispatcher yet.

So the brief's "put a 3-column table in A1:C4" (an Excel-range Office.js op) is
**not implemented on any platform on this branch** — it would return
`OFFICE_PANE_REQUIRED`. The only real tool round-trips available are the two
osascript-backed tools.

---

## Unknown (2) — osascript / Apple Events  →  NEEDS-WORKAROUND → FIXED

Ran the exact `list_script()` output against live Word + Excel (both had documents open):

```
Word:  execution error: every document doesn't understand the "count" message. (-1708)
Excel: execution error: Parameter error. (-50)
```

**Root cause:** `repeat with d in {collection}` makes AppleScript send a `count`
to the element specifier, which Word/Excel's dictionaries reject. **Fix:** materialize
the list first with `repeat with d in (get {collection})`. After the fix both apps
enumerate correctly (verified: name / full name / saved / active tab-separated lines).

Second bug, in `act_word_blocking()`:
- Same `repeat with d in documents` → `(get documents)`.
- `save theDoc` on a **never-saved** document pops a blocking GUI "Save As" dialog
  that hangs the osascript call indefinitely (observed live). **Fix:** guard with
  `if (path of theDoc) is not "" then save theDoc`. The content append + read-back
  body itself works (verified: `ZIEE_TEST_LINE` appended + read back).

Both fixes committed to `platform/macos.rs` on `feat/office-bridge` (with the empirical
error codes recorded in the code comments).

Note: PowerPoint (not running) returned `-10003 "Access not allowed"` — a TCC
Automation-consent artifact, harmless because the `is running` guard short-circuits;
first real use will trigger the standard per-app Automation consent prompt.

---

## Unknown (1) — cert trust  →  WORKS (system-trust layer)

Minted a self-signed `localhost` CA cert with SANs `DNS:localhost, IP:127.0.0.1, IP:::1`
(matching DEC-5 / `cert.rs`), then ran the **exact** `install_cert_trust` command:

```
security add-trusted-cert -d -r trustRoot -k ~/Library/Keychains/login.keychain-db cert.cer   →  rc=0
```

Then the system trust evaluator (the same `SecTrust` path WKWebView/ATS use):

```
security verify-cert -c cert.cer -p ssl -s localhost   →  "certificate verification successful."  rc=0
```

After removal, the same cert correctly evaluates `CSSMERR_TP_NOT_TRUSTED`, confirming
the trust came from our added anchor. Test cert fully removed; keychain restored.

**Caveat / open item:** `-d` targets the admin trust domain and typically raises a
one-time GUI admin-auth dialog. rc=0 here (a dialog may have been auto-approved). The
DEC-9 comment's specific worry — does the **WKWebView** task pane honor a user-added
trustRoot prompt-free — is *strongly* indicated (WKWebView uses SecTrust, which now
passes) but not yet confirmed in-app; that needs the running desktop app + open pane.

---

## Unknown (3) — full round-trip  →  PARTIAL

Verified without the app:
- **Sideload:** all three sandbox containers
  (`~/Library/Containers/com.microsoft.{Word,Excel,Powerpoint}/Data/Documents/`) exist
  and are writable; `register_sideload`'s copy lands `manifest.xml` in each `wef/`.
  Confirmed the deliberate lowercase `com.microsoft.Powerpoint` is the real container
  id (`MCMMetadataIdentifier`), and the mixed-case path is just a case-insensitive-APFS
  alias to the same dir — so the code's lowercase choice is correct.
- **Manifest:** hardcodes `https://localhost:44300` for `SourceLocation`/`AppDomain`,
  so the bridge must bind the fixed port 44300 (it does).

Pending the running app (filled in below): bridge binds 44300 HTTPS+WSS on loopback,
`/taskpane.html` served over rustls with injected token, `/bridge` WSS handshake
accepts a valid token+origin and echoes, task pane loads in the Office WKWebView
prompt-free, and the two osascript tools round-trip through a real chat.

## Build blocker fixed (pre-existing, unrelated to office_bridge)

The desktop crate would not compile on macOS at all:

```
error[E0592]: duplicate definitions with name `verify_loopback_bind`
  server/src/modules/llm_local_runtime/deployment/local.rs:460 (#[cfg(target_os = "macos")])
  server/src/modules/llm_local_runtime/deployment/local.rs:802 (#[cfg(not(any(target_os="linux", target_os="windows"))) — includes macOS!])
```

The macOS-specific `verify_loopback_bind` (added later) collided with the catch-all
fallback whose `cfg` was never tightened to exclude macОS. Fixed by adding
`target_os = "macos"` to the fallback's `not(any(...))`. Committed on `feat/office-bridge`.
This is a general macOS-build bug, not an office_bridge bug — but it blocks any Mac desktop build.

## RUNTIME — desktop app running (embedded server + bridge)

Launched `target/debug/ziee-desktop`. Boot log:
```
ziee backend server started successfully on 127.0.0.1:8080
office_bridge: bridge listening on https://localhost:44300 (dual-stack; cert fp ff2fb11e…)
office_bridge: built-in server 8d208f31-… registered at http://127.0.0.1:8080/api/office-bridge/mcp
office_bridge: open/close watch loop started (user=…, tick 4s)
```

Verified against the LIVE bridge:
- **Dual-stack bind:** `lsof` shows LISTEN on BOTH `127.0.0.1:44300` and `[::1]:44300`. ✓
- **Task pane over rustls:** `GET https://localhost:44300/taskpane.html` → 200, with a
  fresh per-session token injected in place of the quoted `"__ZIEE_BRIDGE_TOKEN__"`
  (the JS var name `window.__ZIEE_BRIDGE_TOKEN__` is correctly left intact). ✓
- **TLS cert SANs:** `CN=localhost`, SAN = `localhost, 127.0.0.1, ::1` — exactly DEC-5. ✓
- **WSS `/bridge` gating + echo:**
  - valid Origin + valid token → **101 Switching Protocols**, subprotocol `ziee-bridge` echoed back. ✓
  - bad Origin → **403**; missing token → **401**. ✓
  - real round-trip: OPEN → sent `ZIEE_ECHO_PING` → received `ECHO_BACK: ZIEE_ECHO_PING` → clean close. ✓

So the same-origin WSS connect-back transport (the hard part of unknown 3's plumbing)
works end-to-end on macOS at the network layer. What remains for a *product* round-trip
is the Office.js pane RPC (ITEM-9, not implemented) — see architecture note above.

## Finding: fixed port 44300 has no occupied-port handling

`bridge/server.rs:102` binds `TcpListener::bind((127.0.0.1, 44300))` with no
fallback. If 44300 is occupied (e.g. a second desktop instance, or any other
process), `start()` errors, the fire-and-forget listener task dies with a log line,
and the task pane silently can't load. The port IS admin-configurable
(`office_bridge_settings.port`) and `materialize_manifest` rewrites the sideloaded
manifest's `:44300` → `:<port>`, so the escape hatch exists — but nothing surfaces
"44300 is in use, change the port" to the user.
**Resolution:** see "Port-collision handling" below — the bridge now emits a typed
`OFFICE_BRIDGE_PORT_IN_USE` and logs a clear single-owner warning instead of dying
silently; it deliberately does NOT migrate/persist a port (shared-settings hazard).
The real multi-instance answer is a shared bridge-broker, deferred to ITEM-9.

---

## What was changed on `feat/office-bridge` (all committed on the branch, NOT main)

1. `platform/macos.rs` — **osascript fix 1**: enumeration uses `repeat with d in
   (get {collection})` (was `repeat with d in {collection}` → `-1708`/`-50` live).
2. `platform/macos.rs` — **osascript fix 2**: Word `act_word_blocking` uses
   `(get documents)` and guards `save theDoc` with `if (path of theDoc) is not ""`
   (unsaved doc → blocking GUI Save-As dialog observed live).
3. `platform/macos.rs` + `platform/unsupported.rs` — **flipped `MAC_TRANSPORT_VERIFIED`
   → true**, updated the two DEC-9 guard tests + all UNVERIFIED doc comments to record
   the passed spike.
4. `server/.../llm_local_runtime/deployment/local.rs` — **macOS build fix**: tightened
   the `verify_loopback_bind` fallback `#[cfg(not(any(linux, windows)))]` to also exclude
   `macos` (duplicate-definition E0592 that blocked ANY desktop build on macOS).

## Remaining work for a full macOS Office feature (NOT done here — out of spike scope)

- **ITEM-9 pane-mediated Office.js RPC** — the 5 non-osascript tools (`get_selection`,
  `add_comment`, `set_track_changes`, `get_tracked_changes`, range writes like the
  brief's "3-column table") still return `OFFICE_PANE_REQUIRED` on every platform. The
  transport they need (WSS + token + echo) is now proven; the JSON-RPC dispatch on top
  of the echo skeleton (`bridge/server.rs::handle_socket`) is the missing piece.
- **Occupied-port UX** for 44300 — now a typed error + single-owner warning (see
  "Port-collision handling" below); full multi-instance sharing is the future broker.
- **TCC Automation consent**: first real osascript use per app raises the standard macOS
  "ziee wants to control Microsoft X" prompt (observed for PowerPoint). Expected; document
  for operators.
- Cosmetic: the runtime app-data bundle id is `com.ziee.chat` — contradicts the
  CLAUDE.md "app is `ziee`, not `ziee-chat`" naming rule (out of scope, flagged).


---

## Port-collision handling (revised — single-owner, no shared-state churn)

An earlier revision auto-migrated the port and persisted it to `settings.port`. That
was withdrawn: the app-data dir is a FIXED path (`com.ziee.chat`), so every ziee
instance reads/writes the **same** `office_bridge_settings` row — a second instance
rewriting `port` would corrupt the port the first instance's sideloaded manifest
depends on. And on a single-user machine, 44300-in-use almost always means *another
ziee already owns the bridge*, so the right response is "don't start a second one,"
not "silently move ports."

Final behavior:
- `bridge/server.rs`
  - `start()` detects `ErrorKind::AddrInUse` and returns the typed code
    `PORT_IN_USE_CODE` (`OFFICE_BRIDGE_PORT_IN_USE`), distinct from a generic bind
    failure that previously read as a silent death.
  - `find_free_loopback_port()` retained (used by the ephemeral-port tests and the
    future broker). Unit tests: `find_free_loopback_port_returns_a_bindable_port`,
    `start_reports_port_in_use_with_distinct_code`.
- `mod.rs::register_office_bridge`: on `PORT_IN_USE`, log a clear warning ("another
  ziee instance likely owns the Office bridge; not starting a second listener") and
  return. **No settings write, no migrate.** The osascript-based tools need no port
  and keep working in that instance regardless.

### Why not "just use a random/free port"?
Office caches the sideloaded manifest's URL and doesn't re-scan `wef/` or reload the
manifest without an app restart, so the bridge port must stay STABLE for the life of
a sideload — a per-boot random port would force a re-sideload + Office restart every
launch. The cert is NOT the constraint (its SAN is `localhost`, port-independent —
one cert covers any localhost port, exactly as `office-addin-dev-certs` works).

### The real multi-instance answer (future, not built here)
Multiple ziee instances **cannot** each own a task pane: Office keys an add-in by its
manifest `<Id>` GUID and allows one instance per Id per Office install. The correct
design for "several ziee instances share Office" is a **single shared bridge-broker**
that owns 44300 + the one sideloaded add-in and multiplexes N instances: each ziee
carries an instance-ID; request→response is routed back to the originating instance by
correlation ID; user-originated Office events broadcast to all. The broker only needs
to multiplex the **ITEM-9 pane path** (the Office.js-over-WSS tools) — the osascript
tools need no broker at all. This is deferred until ITEM-9's pane RPC is actually
built (today the WSS `/bridge` is an echo skeleton). Parallel *testing* already avoids
all of this: bridge integration tests bind an ephemeral port + tempdir cert, and the
E2E mocks the Office boundary — neither touches 44300 or real Office.

---

## ITEM-9 pane RPC — LIVE Mac verification (TEST-13)

The daemon↔pane JSON-RPC (the 5 pane-mediated Office.js tools) was verified live on
this Mac, driving real Office.js in the actual Excel WKWebView task pane via the
`#[ignore]` harness `test13_live_mac_pane_ops` (binds 44300, reuses the trusted cert).

Pane log (real Excel task pane):
```
Office.onReady host=Excel platform=Mac
addHandler status=succeeded
bridge open (host=Excel, token=present)
registered (doc_key=<unsaved>)
```
Harness log (the daemon side, driving ops through the pane):
```
pane 2 registered (host=Excel, doc_key_set=false)
pane connected (target = "Untitled"); driving ops...
get_selection returned: {"text":"hello ziee"}
read_document returned: {"text":"hello ziee","truncated":false}
TEST-13 LIVE PASS: both ops round-tripped through the real Office pane.
test result: ok. 1 passed; 0 failed
```

`get_selection` and `read_document` both executed Office.js in the live pane and
returned the real `hello ziee` cell content. Combined with the earlier transport spike
(WKWebView prompt-free load + WSS connect-back) and the mock-pane integration tests
(the exact wire contract), the macOS pane path is verified end-to-end. Notes: the pane
connects the instant a real Office document is open (the earlier harness misses were
simply Excel being closed); the `no-store` token-page fix ensures a restarted bridge is
never handed a stale cached token.
