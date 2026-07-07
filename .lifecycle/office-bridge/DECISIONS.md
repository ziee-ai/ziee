# Office Bridge — DECISIONS

Every product/technical input resolved before implementation. Basis ∈ user | convention | codebase.

### DEC-1: Windows Office automation — `windows` crate COM vs raw `windows-sys`?
**Resolution:** use the `windows` crate (late-bound `IDispatch`, `GetActiveObject`, `VARIANT`, oleacc `AccessibleObjectFromWindow`, `EnumWindows`), `#[cfg(windows)]`-scoped, alongside the existing `windows-sys`.
**Basis:** user — explicitly chosen at plan time; `windows-sys` is raw FFI with no COM helper types, making late-bound IDispatch impractical.

### DEC-2: Cert trust install path?
**Resolution:** `certutil -addstore -f Root <cert.cer>` into LocalMachine\Root via an elevated `ShellExecute("runas")` — one UAC during `[Connect]`.
**Basis:** user — chosen at plan time; proven silent + honored by WebView2 for all users in the spike.

### DEC-3: When is the module (and its bridge listener) active?
**Resolution:** `probe()`-gated — `init()` registers the `mcp_servers` row and spawns the bridge + daemon only when the host is a supported desktop OS with Office present; otherwise it logs the reason and skips entirely.
**Basis:** codebase — mirrors `code_sandbox` `probe_host` (headless/Linux servers must not bind 44300 or attempt COM).

### DEC-4: Does the office tool bypass per-call approval?
**Resolution:** no. Add the flag→id branch to `auto_attach_builtin_ids` only; leave `is_builtin_server_id` unedited so mutating operations (`edit_document`/`add_comment`/`set_track_changes`) require per-call approval.
**Basis:** codebase — `control_mcp` (also mutating) is deliberately absent from `is_builtin_server_id`.

### DEC-5: Bridge port + bind addresses?
**Resolution:** fixed TCP **44300**, bound dual-stack on `127.0.0.1` AND `[::1]`; cert SAN includes both plus `localhost`.
**Basis:** convention — proven load-bearing (WebView2 resolves localhost→`::1`); a fixed port lets the manifest `SourceLocation` be static.

### DEC-6: How does the task pane obtain its per-session token?
**Resolution:** the daemon stamps a fresh session token into the served `taskpane.html` at request time (inlined constant); the pane sends it in the WSS connect (subprotocol/first frame). The bridge rejects any WSS/POST without a valid token or with an Origin ≠ `https://localhost:44300`.
**Basis:** convention — same-origin served page + per-session token mirrors the `proxy.rs` token model; avoids a URL-query token that could leak via logs.

### DEC-7: Sync audience for open/close document events?
**Resolution:** `Audience::owner(user_id)` on `SyncEntity::OfficeDocument`, with the client store self-gating its refetch on `office_bridge::use`.
**Basis:** codebase — sync module rule: owner-scope per-user data; never `everyone()`; the self-gate perm equals the refetch endpoint's read perm.

### DEC-8: `office_bridge_settings` schema (singleton)?
**Resolution:** `id BOOLEAN PRIMARY KEY DEFAULT TRUE CHECK (id = TRUE)`, `enabled BOOLEAN NOT NULL DEFAULT TRUE`, `port INTEGER NOT NULL DEFAULT 44300`, `last_connected_at TIMESTAMPTZ NULL`, `cert_fingerprint TEXT NULL`.
**Basis:** codebase — the `web_search_settings`/`code_sandbox_settings` singleton pattern.

### DEC-9: macOS scope in this pass?
**Resolution:** compile the `#[cfg(target_os="macos")]` `OfficePlatform` scaffold (AppleScript/`security`/wef-folder) behind `const MAC_TRANSPORT_VERIFIED: bool = false`; every path annotated `// UNVERIFIED — Mac spike`. Do not claim macOS works; the gate is the Keychain-trust + WKWebView same-origin-WSS round-trip spike.
**Basis:** user — the handoff mandates Windows-full + macOS-scaffold-gated; this box cannot runtime-verify macOS.

### DEC-10: TLS + WebSocket stack for the standalone bridge listener?
**Resolution:** axum `Router` with the `ws` feature for `/bridge`, served over rustls via `axum-server`'s `bind_rustls`, one bind per address for dual-stack. New deps: `rcgen`, `axum-server`, `rustls`, axum `ws` feature.
**Basis:** convention — ziee has no existing axum WS/TLS; axum-server+rustls is the standard idiomatic pairing and keeps the bridge a small self-contained Router.

### DEC-11: Which crate hosts the module — server or tauri?
**Resolution:** the server crate (`modules/office_bridge/`), which the Tauri desktop app runs in-process at the user's (non-elevated, in-session) integrity — satisfying the COM same-integrity requirement — while `probe()` disables it on headless/server deploys.
**Basis:** codebase — built-in MCP modules live in the server crate; desktop-gating is done via runtime probe, not a build feature.

### DEC-12: Enable / kill-switch model?
**Resolution:** a config-level deploy kill-switch (`office_bridge: { enabled: false }`) that skips registration, plus the runtime `office_bridge_settings.enabled` admin toggle that gates attachment — two independent levels.
**Basis:** codebase — mirrors `web_search`/`lit_search` (deploy kill-switch distinct from the runtime settings toggle).
