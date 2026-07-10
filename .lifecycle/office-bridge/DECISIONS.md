# DECISIONS — office-bridge (consolidated)

All up-front decisions from the five stages, renumbered. No unresolved decisions remain.

## Stage: Foundation — module, settings, watcher, bridge listener

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

## Stage: Pane RPC — daemon↔pane JSON-RPC broker + 5 pane tools

### DEC-13: How is `doc_full_name` (native list) mapped to a connected pane (`doc_key`)?
**Resolution:** each pane registers a `doc_key` from `Office.context.document.url`.
`call_pane` resolves in order: (a) a pane whose `doc_key` equals `doc_full_name` or
shares its basename (path formats differ between the native enumerate and Office.js),
(b) if exactly one pane is connected, that sole pane, (c) otherwise a typed
`OFFICE_PANE_NOT_CONNECTED`. The common case (one open task pane) always resolves.
**Basis:** convention — best-effort correlation; a suffix/basename match tolerates the
`/Users/x/Report.docx` (native) vs `file:///…/Report.docx` (Office.js) format gap.

### DEC-14: `call_pane` timeout?
**Resolution:** a 15s wall-clock timeout via `tokio::time::timeout` around the oneshot
recv, exposed as a `const` with a test-only shorter override path. Timeout →
`OFFICE_PANE_TIMEOUT` and the pending entry is removed.
**Basis:** convention — generous for an interactive Office.js op on a large document,
in the same tens-of-seconds band as other interactive tool waits; deterministic tests
use a short override so they don't sleep 15s.

### DEC-15: Which hosts support each pane op, and how is an unsupported combo reported?
**Resolution:** `get_selection` + `read_document` → Word + Excel (PowerPoint
best-effort selection); `add_comment` + `set_track_changes` + `get_tracked_changes` →
Word only. `taskpane.js` executes the op only on a supporting host and otherwise replies
with a JSON-RPC `error` the backend maps to `OFFICE_UNSUPPORTED_ON_HOST`. The existing
**native** PowerPoint pre-gate for `add_comment`/`set_track_changes` stays (fast path,
no round-trip).
**Basis:** codebase + API reality — mirrors the existing `OFFICE_UNSUPPORTED_ON_HOST`
capability model; comments / change-tracking are Word Office.js APIs, not Excel/PPT.

### DEC-16: `read_document` content shape per host?
**Resolution:** Word → `body.text` (plain text). Excel → the used range serialized as
TSV (rows by `\n`, cells by `\t`), capped at a sane character budget. Both returned in
the `{ text }` result the tool descriptor promises ("document body as plain text").
**Basis:** convention — TSV is the natural plain-text projection of a sheet; matches the
descriptor's "plain text" contract.

### DEC-17: Does the pane's `register` frame need auth beyond the WSS upgrade?
**Resolution:** no. The `/bridge` upgrade already validated the per-session token +
Origin, so a post-upgrade `register` frame is trusted; the broker keys the pane by its
socket/connection, not by a re-presented token.
**Basis:** codebase — mirrors DEC-18 of the original feature (socket authenticated at
upgrade; the frame-level `session_token` is belt-and-suspenders, not load-bearing for
routing).

### DEC-18: Correlation-id space — shared with the pane's ids or separate?
**Resolution:** the daemon allocates its own monotonic `AtomicU64` corr ids for
daemon→pane requests; responses are matched ONLY against the daemon's `PENDING` map.
A pane→daemon frame is classified as a request/notification by the presence of `method`
(never looked up in `PENDING`); a daemon→pane response is classified by `result`/`error`.
No cross-direction id collision is possible.
**Basis:** convention — JSON-RPC 2.0 disambiguates direction by method-vs-result/error;
`protocol.rs` already models both shapes.

### DEC-19: Pane→daemon events (`selection_changed`) — broadcast or keep as-is?
**Resolution:** keep the current behavior (debug-log, not consumed). The model reads the
selection via the `get_selection` **pull** (request/response), which fully covers the
requirement. Broadcasting spontaneous pane events to (multiple) ziee instances is the
future shared-broker concern and is explicitly out of scope here.
**Basis:** user scope — the requested work is the 5 request/response tools; event
fan-out is deferred and documented (broker design note).

### DEC-20: Concurrent ops to the same pane — serialize?
**Resolution:** no explicit serialization. Each op carries a unique corr id; the pane
processes `Word.run`/`Excel.run` batches independently and the broker correlates
replies by id, so interleaved in-flight ops are safe.
**Basis:** convention — Office.js run batches queue internally; id correlation handles
interleaving without a broker-side lock.

### DEC-21: New file `broker.rs` vs folding the broker into `server.rs`?
**Resolution:** a new `bridge/broker.rs` owning the two registries + `call_pane`;
`server.rs::handle_socket` calls into it. Keeps the socket I/O and the correlation
state separate + independently unit-testable (the broker unit tests need no socket).
**Basis:** codebase — mirrors the `auth.rs` (state module) vs `server.rs` (I/O) split
already in `bridge/`.

## Stage: run_office_js — open-ended Office.js pane surface

### DEC-22: What timeout governs a `run_office_js` pane round-trip?
**Resolution:** Reuse the shared `broker::CALL_TIMEOUT` (15s). A script that
exceeds it surfaces the existing typed `OFFICE_PANE_TIMEOUT`, which the model
self-corrects on by reducing scope (smaller batch). No new/separate timeout in
v1; a configurable per-call timeout is a documented future enhancement.
**Basis:** convention — reuse the existing broker timeout (DEC-23 of the prior
pane-rpc feature); the writeup's "structured errors so the model self-corrects"
makes the timeout a recoverable signal, not a hard failure.

### DEC-23: Does `run_office_js` need any per-call-approval gating code?
**Resolution:** No. `office_bridge` is deliberately ABSENT from
`is_builtin_server_id` (server `mcp/chat_extension/mcp.rs:122-127, 204-221`), so
every office_bridge tool — including `run_office_js` — already requires per-call
user approval. Do NOT add office_bridge to the approval-bypass set. The security
requirement is satisfied by the existing posture with zero new code.
**Basis:** codebase — the auto-attach-but-not-bypass seam is explicit and
commented for exactly this ("mutating office tools stay behind per-call approval").

### DEC-24: Remove `edit_document` entirely, or keep it as a native no-pane append?
**Resolution:** Remove `edit_document` (and its `op` enum + the sole-consumer
native `act_on_document`/`DocOp`/`ActResult` path). `run_office_js` subsumes
append (`context.document.body.insertParagraph(text, "End")`). Accepted
trade-off: this drops the ONE capability that worked without an open task pane
(native osascript/COM append); acceptable because the entire pane-mediated
surface (read/selection/comments/track) already requires a pane, so the bridge's
value proposition already assumes one.
**Basis:** user — the handoff writeup is explicit: "Delete the `edit_document`
`op` enum … simplest is to drop the enum and let edits go through run_office_js."

### DEC-25: Ship the read/write declared-intent split (auto-approve read scripts) in v1?
**Resolution:** Defer. v1 ships write-always-prompts — `run_office_js` always
requires per-call approval (inherited, DEC-23), no `mode: read|write` parameter,
no context-proxy read-only enforcement. A future item can add the declared-intent
split + proxy enforcement.
**Basis:** user — the writeup: "If that's too much for v1, ship write-always-
prompts and defer the split."

### DEC-26: How is the host runtime chosen, and what about an unknown host?
**Resolution:** Pick `Word.run` / `Excel.run` / `PowerPoint.run` by the pane's
`HOST` global (set on `Office.onReady`). `run_office_js` is host-agnostic — NO
PowerPoint pre-gate (unlike the Word-only comment/track tools). An unknown /
undefined host (or the matching `*.run` API absent) → `replyErr(id,
ERR_UNSUPPORTED_HOST, …)` (−32002), which the daemon maps to
`OFFICE_UNSUPPORTED_ON_HOST`.
**Basis:** codebase — mirrors `opReadDocument`'s `HOST`-branching and the
existing `ERR_UNSUPPORTED_HOST` → `OFFICE_UNSUPPORTED_ON_HOST` mapping.

### DEC-27: What real LLM backs TEST-48, and how is it gated?
**Resolution:** The OpenAI-compatible LiteLLM proxy on `coder.ziee:4000`
(model `qwen3.6-35b-a3b`; verified it emits a well-formed `run_office_js` tool
call with valid Office.js). Reached from this Mac via an SSH tunnel
(`ssh -fN -L 4000:127.0.0.1:4000 coder.ziee`). TEST-48 reads
`ZIEE_OFFICE_REAL_LLM_URL` (the OpenAI `/v1/chat/completions` base), with optional
`ZIEE_OFFICE_REAL_LLM_MODEL` (default `qwen3.6-35b-a3b`) and
`ZIEE_OFFICE_REAL_LLM_KEY` (LiteLLM key if required). Soft-skips (eprintln +
early return) when the URL env var is unset, mirroring `injection_test`'s
`ANTHROPIC_API_KEY` soft-skip, AND is `#[cfg(target_os="macos")] #[ignore]` since
it drives a live Excel pane. Scope note: TEST-48 calls the model directly (real
model + real shipped tool schema + real Office.js execution via the live pane);
it does NOT route through ziee's OpenAI-provider chat pipeline (the desktop test
harness has no chat-send helper, and building one is out of this feature's scope)
— the daemon-side tool routing is covered non-LLM by TEST-44.
**Basis:** user — instructed to use the `coder.ziee` `:4000`/`:8000` endpoint for
the real-LLM test; soft-skip pattern from the codebase.

### DEC-28: What is the `run_office_js` result shape, and how are big/odd returns handled?
**Resolution:** The pane replies `{ result, truncated }` — `result` is the
script's `return` value JSON-serialized then capped via the existing
`capText`/`MAX_READ_CHARS`; `truncated` flags the cap. No return / `undefined` →
`result: null`, `truncated: false`. A non-JSON-serializable / circular value
degrades to `String(value)` and never throws. The daemon wraps this via
`pane_tool_result` (result in `structuredContent`, readable text in `content`).
**Basis:** convention — mirrors `read_document`'s `{ text, truncated }` cap and
the `pane_tool_result` wrapper.

### DEC-29: How does the pane execute the model-supplied script string?
**Resolution:** Compile it as an async function body —
`new Function('context', '"use strict"; return (async function(){' + script +
'\n})()')` — and invoke it INSIDE the host `run`:
`Excel.run(function(context){ return theFn(context); })` (and Word/PowerPoint).
The script may `await context.sync()` and `return` a value; `run()` auto-syncs on
resolve and rolls back on throw. The trailing `\n` guards a `//`-comment last line.
**Basis:** convention — the standard Office.js embedding; the `context` object is
exactly what `opReadDocument` uses inside `Word.run`/`Excel.run`.

### DEC-30: What structured error does a failed script return?
**Resolution:** On `catch(e)`, `replyErr(id, ERR_OP_FAILED, msg)` where `msg`
combines `e.name`/`e.message` and, when the error is an `OfficeExtension.Error`,
its `.code` and `.debugInfo` (JSON-stringified). The daemon surfaces this as
`OFFICE_PANE_ERROR`, giving the model the failing code + context to self-correct
in one retry.
**Basis:** user (writeup: "Return STRUCTURED ERRORS … so the model self-corrects
in ONE retry") + codebase (the existing `.catch → replyErr` pattern).

## Stage: Desktop-only relocation — module moved server→desktop

### DEC-31: How is the module registered from the desktop crate — cross-crate `#[distributed_slice]`, or a runtime register seam?
**Resolution:** **RUNTIME SEAM** (revised after Phase-5 codebase evidence). Reshape office_bridge to a `host_mount`-style `DesktopModule` that, at boot, registers its REST/settings routes (`register_api_routes`), spawns the bridge listener + watcher and upserts the `mcp_servers` row using `ziee::Repos.pool()`, and registers its built-in MCP server + chat-extension + auto-attach entry via new `ziee::register_*` runtime functions. NO cross-crate `#[distributed_slice(ziee::…)]`.
**Basis:** codebase — the ONLY existing desktop→server downstream-registration in the repo is a runtime seam (`host_mount/mod.rs:82` → `ziee::code_sandbox::register_sandbox_mount_provider`, `mount_provider.rs:62`); there are ZERO `#[distributed_slice(ziee::…)]` registrations in the desktop crate. The "match existing patterns" project rule ([[feedback_match_existing_patterns]]) makes the runtime seam decisive — it is guaranteed to work and avoids linkme's cross-crate dead-code-linkage caveat, which is unproven here.

### DEC-32: How is the chat extension (and the auto-attach entry) registered from the desktop crate?
**Resolution:** Via runtime register functions in `ziee`, mirroring `register_sandbox_mount_provider`: `ziee::register_chat_extension(entry)` and `ziee::register_auto_attach_builtin(AutoAttachEntry)`, each appending to a `OnceLock<Mutex<Vec<…>>>` registry that the server consumes at boot (the chat-extension registry alongside `CHAT_EXTENSIONS`; `auto_attach_builtin_ids` alongside its slice/hardcoded arms). office_bridge's `DesktopModule::init` calls both. The `AUTO_ATTACH_BUILTINS` distributed slice added in Phase 5 (`e4776d6a`) is converted to / augmented by this runtime registry.
**Basis:** codebase — same `register_sandbox_mount_provider` precedent; guaranteed cross-crate delivery without linkme.

### DEC-33: Does the `AUTO_ATTACH_BUILTINS` inversion move ALL built-ins off the hardcoded list, or only office_bridge?
**Resolution:** Introduce the `AUTO_ATTACH_BUILTINS` distributed slice and iterate it IN ADDITION to the existing hardcoded arms in `auto_attach_builtin_ids`; move ONLY office_bridge's `{flag → server_id}` entry into it (registered from the desktop crate). Other built-ins keep their current handling.
**Basis:** convention — minimal blast radius ([[feedback_match_existing_patterns]]); only office_bridge's entry MUST leave `ziee` for the server to compile without the module.

### DEC-34: Does the `OfficeBridgeConfig` kill-switch move to the desktop, or stay in `ziee::Config`?
**Resolution:** Stays in `ziee::Config` as the existing `Option<OfficeBridgeConfig>` section (inert in server builds; read by the module via `ziee::Config`).
**Basis:** codebase — avoids churning the `Config` struct/schema; an unused optional section is harmless and the module already reads it through `ziee::Config`.

### DEC-35: Does `SyncEntity::OfficeDocument` move out of `ziee`?
**Resolution:** No — the variant stays in `ziee`'s `SyncEntity` enum; the desktop module references `ziee::SyncEntity::OfficeDocument`.
**Basis:** codebase — a downstream crate cannot add an enum variant; the enum drives the generated frontend `SyncEntity` TS union; the variant is inert (never emitted) in server builds.

### DEC-36: What migration numbers do office_bridge's migrations take in the desktop dir?
**Resolution:** `10000000000006_create_office_bridge.sql` + `10000000000007_grant_office_bridge_permissions_to_users.sql` (deleted from `server/migrations`). Fix the cosmetic "migration 133" mislabel in the grant's comment/warning.
**Basis:** codebase — next free in the desktop `1000…` space; disjoint from the server `0000…` space; grant runs after all server migrations so the Users group row exists.

### DEC-37: Where do the integration tests live and on which harness?
**Resolution:** `src-app/desktop/tauri/tests/office_bridge/`, on the desktop `TestServer` harness; if the desktop harness lacks `create_user_with_permissions`-style helpers the server harness has, add the minimal equivalent there.
**Basis:** codebase — `desktop/tauri/tests/` `host_mount_tests` is the precedent for a desktop-module integration suite.

### DEC-38: Does the WEB UI keep any office-bridge stub after the module moves to `desktop/ui`?
**Resolution:** No — the office-bridge UI module is fully removed from `src-app/ui` (dir moved to `desktop/ui`, dropped from the web UI module registry + e2e). The web app carries zero office-bridge.
**Basis:** user — the feature must be desktop-only.

### DEC-39: Does office_bridge stay behind per-call approval after the inversion?
**Resolution:** Yes — office_bridge stays OUT of `is_builtin_server_id`, and its `AUTO_ATTACH_BUILTINS` entry carries NO approval-bypass. Mutating office tools remain gated by approval.
**Basis:** codebase — preserves the original office_bridge feature's DEC-34 (mutating tool behind approval).

### DEC-40: This transforms the existing (already-committed) server-crate office_bridge — move or duplicate?
**Resolution:** A MOVE (git delete-from-server + add-to-desktop, rename-detected). The original `.lifecycle/office-bridge/` artifacts remain committed as branch history; this `office-bridge-desktop-only` feature dir tracks the re-architecture; both are stripped at merge to main per lifecycle hygiene.
**Basis:** convention — lifecycle artifacts are per-feature process records; product code is transformed, not duplicated.

## Stage: Mode-gated approval — read auto-runs, write prompts

### DEC-41: Is `mode` required, and what happens if it's missing/garbled?
**Resolution:** `mode` is declared **required** in the `run_office_js` `inputSchema`
(enum `["read","write"]`), but the server approval logic is **fail-safe**: only an
EXACT `"read"` bypasses; a missing, null, non-string, or any-other-string `mode` is
treated as `write` → prompt. The daemon never rejects a call for `mode` (it ignores
`mode` entirely — see DEC-44), so a model that forgets `mode` still executes after a
write approval rather than erroring.
**Basis:** user (trust-the-model design) + convention (fail-safe: unknown ⇒ the
safe/approved path, exactly like `control`'s "unknown op ⇒ approve").

### DEC-42: The exact read-bypass predicate.
**Resolution:** A call bypasses approval iff ALL THREE hold: the server id ==
`office_bridge_mcp_server_id()`, the tool name == `"run_office_js"`, and
`input["mode"]` is the exact string `"read"`. Any other combination → normal approval.
**Basis:** user + security — gating on the server id (not tool-name alone) makes a
user-added MCP server that happens to name a tool `run_office_js` unable to obtain
auto-approval (spoof-safe); the exact-string match makes `"READ"`/`"read "`/junk fail
safe (TEST-76 pins every case).

### DEC-43: Where does the classifier live?
**Resolution:** A new server module `src-app/server/src/modules/mcp/chat_extension/office_approval.rs`
holding `office_bridge_mcp_server_id()`, `run_office_js_read_bypass(...)`, the extracted
`compute_needs_approval(...)`, and their `#[cfg(test)]` unit tests. `mcp.rs` calls it.
**Basis:** convention — mirrors `control_mcp/handlers.rs` (classifier + pure decision +
in-source tests, called from the approval loop).

### DEC-44: Does the daemon (`dispatch_tool`) read or validate `mode`?
**Resolution:** No. `mode` is consumed ONLY by the server approval loop. The desktop
daemon and the pane ignore it entirely — execution of a `run_office_js` script is
byte-identical regardless of `mode`. There is NO pane-side read-only enforcement
(no Proxy). The `run_office_js` dispatch arm is unchanged by this feature.
**Basis:** user — explicit decision to trust the declared mode rather than build the
enforced read-only Proxy ("that's a lot to implement then, let LLM decide it").

### DEC-45: Signature of the extracted decision, and behavior-preservation.
**Resolution:** `compute_needs_approval(server_id: Uuid, tool_name: &str, input: &Value,
approval_mode: ApprovalMode, is_builtin: bool, is_control: bool, auto_approved: bool)
-> bool` — returns the `needs_approval` boolean, reproducing the current `mcp.rs:2120`
if/else EXACTLY (control → `control_call_needs_approval`; builtin → false; else per
`ApprovalMode`, honoring the precomputed `auto_approved`), with ONE added arm before
the `else`: office_bridge `run_office_js` `read` → false. The Disabled-mode → deny
early-continue at `mcp.rs:2103` is left in place unchanged (it precedes this decision).
`auto_approved` is passed in precomputed (the existing `auto_approved_servers`/
`user_auto_approved` `contains_tool` check) so the pure function stays DB-free.
**Basis:** codebase — minimal behavior-preserving extraction; TEST-78 pins all 9 branches.

### DEC-46: How does the server get office_bridge's id without depending on the desktop crate?
**Resolution:** `office_bridge_mcp_server_id()` recomputes the same deterministic id the
desktop registers: `Uuid::new_v5(&Uuid::NAMESPACE_URL, b"office_bridge.ziee.internal")`.
A desktop-crate test (`mod.rs`, TEST-77) asserts the desktop `office_bridge` row id ==
`ziee::…::office_bridge_mcp_server_id()` so the two derivations can never drift (the
desktop crate depends on the server lib, so it sees both).
**Basis:** codebase — deterministic v5 ids are already the module convention
(`office_bridge.ziee.internal` is the exact string the desktop `mod.rs` uses).

### DEC-47: What backs the real-LLM end-to-end test (TEST-83)?
**Resolution:** The OpenAI-compatible LiteLLM proxy on `coder.ziee:4000`
(`qwen3.6-35b-a3b`) via the SSH tunnel, env-gated on `ZIEE_OFFICE_REAL_LLM_URL` with a
soft-skip when unset. It uses a MOCK office MCP server (registered under
`office_bridge_mcp_server_id()` exposing `run_office_js`), so it needs NO live Excel
pane — it verifies only that a real model picks `mode` correctly and the approval loop
gates accordingly.
**Basis:** user (prior instruction to use the `coder.ziee` endpoint) + codebase
(soft-skip pattern from `injection_test`).

### DEC-48: Recorded accepted security trade-offs (trust-based model).
**Resolution:** Shipping the trust-the-declared-mode model WITHOUT read-only
enforcement is a conscious choice with three accepted risks, documented in
`OFFICE_TOOL_SURFACE_DESIGN.md`: (1) a prompt-injected `mode:"read"` script that
actually mutates bypasses approval (no enforcement); (2) auto-approved reads are a
silent full-document-content exfiltration channel (auditable via `mcp_tool_calls`, not
blocked); (3) "always allow" grants every subsequent write in the conversation,
including a later injected one. The enforced read-only Proxy remains a documented
future option if the threat model tightens.
**Basis:** user — explicitly weighed and accepted after the read-op/enforcement analysis.
