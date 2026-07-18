# ZIEE_CHAT_AGENT_CORE default-flip readiness map

Goal: what must be true before `ZIEE_CHAT_AGENT_CORE=1` can be the **default**
(today it is opt-in; legacy is default). Each blocker: **severity** · **fix plan**
· **status**. Scopes Phase B. Guardrails unchanged (no merge/push/flip until Khoi
signs off).

Legend — severity: **HIGH** (blocks flip), **MED** (should fix before flip),
**LOW** (nice-to-have / accept). Status: `open` / `in-progress` / `fixed` /
`accept` / `verify-manually`.

---

## 1. Functional gaps

### B1 — files_mcp write tools don't attach in an empty conversation  · HIGH · open
- **What:** `attach_files_mcp` is set only when
  `manifest_available = !files.is_empty()` (`file/available_files.rs:292` →
  `file/chat_extension/file.rs:117`). So in a conversation with **no files yet**,
  the ENTIRE files_mcp tool set — including the write tools
  `create_file`/`edit_file`/`rewrite_file`/`convert_document` — never attaches.
  The model cannot author the first file (chicken-and-egg). Breaks 8
  `agentic_chat::*` tests; a flagship "ask the model to write me a doc" flow fails.
- **Not agent-core-specific:** the gate lives in the SHARED file chat-extension,
  so it fails identically on OFF and ON (flag-invariant, proven by isolation:
  `multi_step_upload_analyze` fails the same line 1514 on both flags). It is a
  general pre-existing bug, but it blocks a good ON-default experience.
- **Why it's currently gated (understood):** `manifest_available` legitimately
  gates TWO things — (a) injecting the file MANIFEST system message, and (b) the
  history recency-drop in `process_content_for_llm` (drop old inlined content only
  when a manifest exists to recover it). Overloading it to ALSO gate tool
  attachment conflated "there are files to describe" with "offer the file tools."
- **Fix plan (Phase B1, behavior-change mini-cycle):** decouple. Attach the
  files_mcp **write** tools whenever the model is tool-capable (regardless of file
  count) so the first file can be authored; keep the **manifest** injection + the
  recency-drop gated on `manifest_available` (unchanged); keep the read-tool
  RELEVANCE sensible (read/grep/semantic_search over zero files simply return
  empty — harmless, or filtered). Add the missing `create_file` StubChat plan arm
  (`tests/common/stub_chat.rs`). The 8 `agentic_chat` tests flip fail→pass on BOTH
  flags — document each. Commit CLEARLY LABELED as a general/pre-existing fix,
  separable from agent-core at merge.

## 2. Latent bugs / security

### B2 — chat ON path skips conversation `disabled_servers` enforcement  · MED · open
- **What:** `ChatToolProvider::call` passes `enforce_conversation_disabled = false`
  (DEC-17, "preserving chat's current non-enforcement") to `call_mcp_tool`, so the
  call-time check that blocks a server/tool the user DISABLED in the conversation
  (`mcp_settings.disabled_servers`, `agent_tool_call.rs:208`) is skipped. Attach-time
  filtering (the shared `before_llm_call`) is the primary defense, but a tool that
  reaches `call_mcp_tool` for a server disabled AFTER attach — or via bare-name
  recovery — is NOT blocked. Security audit flagged it as "ignore the user's disable."
- **Fix plan (Phase B2):** pass `enforce_conversation_disabled = true` from the chat
  host (behavior change: the ON path now honors the user's disable at call time, as
  defense-in-depth on top of attach-time filtering). Add an integration test: disable
  a server mid-conversation, drive a tool call at it, assert it is refused. Verify the
  common path (built-ins / non-disabled servers) is unaffected. Flag-invariant risk is
  low: only user-disabled servers are affected, and an attached (non-disabled) tool
  passes the check.

### B3a — `call_mcp_tool` parses server_name as UUID first  · LOW · accept→guard
- **What:** a workflow tool step whose external server is NAMED a literal UUID takes
  the raw-id path instead of name resolution. Fails safe today (accessibility
  re-validation → forbidden/404; no cross-user reach). Currently `accept-with-rationale`
  in the ledger.
- **Fix plan (Phase B3):** add a cheap guard so the raw-id branch is taken only when the
  caller intends an id (chat scheme), keeping workflow name resolution unambiguous.

### B3b — redundant `manager::global()` lookup  · LOW · accept/fold
- **What:** `manager::global()` (a cheap process-global `OnceLock` read) is looked up in
  both `call_mcp_tool` and the provider methods. `accept-with-rationale` today.
- **Fix plan (Phase B3):** fold where a single scope exists; otherwise keep the accept.

## 3. ON-vs-legacy parity (largely ESTABLISHED)

All items below were fixed + verified across the audit rounds; listed so the flip
decision has the full parity ledger.

- **Approval flow** — single-use claim + rotation grace; `is_trusted` strictly tighter
  than legacy (code_sandbox/control go through review). · fixed · `verify-manually` (C1)
- **MCP sampling round-trip** — ephemeral `new_with_sampling` session on the chat path. ·
  fixed · `verify-manually` (C1)
- **Tool-call journaling** — `mcp_tool_calls` rows carry branch/message/tool_use; `source=chat`. ·
  fixed · `verify-manually`
- **tool_use id uniqueness** — `UniquifyingModelClient` rewrites reused ids. · fixed
- **Cross-turn transcript** — persisted; multi-turn recall test green. · fixed · `verify-manually` (C1)
- **Summarization / core-memory** — chat's summarization runs as a chat extension via the
  `RegistryBridge` (`before_llm_call` each iteration); the dispatcher's `CompactionExtension`
  is a 200k failsafe only. · parity-by-construction · `verify-manually` (C1)
- **Stop/cancel** — chat stop token bridged into the crate `CancelToken`
  (`dispatcher.rs:211`). · fixed · `verify-manually`
- **Two-flag regression** — chat:: + mcp:: ON == OFF on the deterministic set (every
  ON-only failure isolation-proven a flake or flag-invariant). · established

## 4. Performance

### P1 — per-iteration extension replay overhead  · LOW · verify-manually
- **What:** the `RegistryBridge` runs the chat `ExtensionRegistry`'s `before_llm_call`
  each loop iteration (as legacy did per model round), plus the agent-core loop
  indirection. No measured regression; expected negligible.
- **Plan:** spot-check turn latency OFF vs ON in Khoi's side-by-side (C1). Not a known
  blocker; flag if a visible latency gap appears.

## 5. Rollout / config surface

### R1 — flip mechanism + escape hatch  · MED · open (post-signoff)
- **What:** the flag is env-based (`ZIEE_CHAT_AGENT_CORE`). "Default = ON" should be a
  deliberate change with an escape hatch (env can still force `0`), and must be
  considered for BOTH the server AND desktop (desktop embeds the server — a flip flips
  desktop too).
- **Plan:** NOT in scope until Khoi signs off (guardrail: no default flip). Recorded so
  the flip PR covers server + desktop + a documented `=0` rollback. Deferred.

---

## Phase B scope (this cycle)
Fix **B1** (files_mcp first-file, flag-invariant general fix), **B2** (disabled_servers
enforcement on chat), **B3** (the 2 LOW nits: guard + fold). Then re-run the two-flag
regression + one blind round; OFF should LOSE the B1 (8 agentic_chat) + B2 failures,
each attributed to its named fix; confirm no NEW deterministic failure and ON still
matches OFF for agent-core behavior. R1 (flip mechanism) is deferred to the post-signoff
flip PR.
