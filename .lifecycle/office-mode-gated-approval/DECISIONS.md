# DECISIONS — office-mode-gated-approval

### DEC-1: Is `mode` required, and what happens if it's missing/garbled?
**Resolution:** `mode` is declared **required** in the `run_office_js` `inputSchema`
(enum `["read","write"]`), but the server approval logic is **fail-safe**: only an
EXACT `"read"` bypasses; a missing, null, non-string, or any-other-string `mode` is
treated as `write` → prompt. The daemon never rejects a call for `mode` (it ignores
`mode` entirely — see DEC-4), so a model that forgets `mode` still executes after a
write approval rather than erroring.
**Basis:** user (trust-the-model design) + convention (fail-safe: unknown ⇒ the
safe/approved path, exactly like `control`'s "unknown op ⇒ approve").

### DEC-2: The exact read-bypass predicate.
**Resolution:** A call bypasses approval iff ALL THREE hold: the server id ==
`office_bridge_mcp_server_id()`, the tool name == `"run_office_js"`, and
`input["mode"]` is the exact string `"read"`. Any other combination → normal approval.
**Basis:** user + security — gating on the server id (not tool-name alone) makes a
user-added MCP server that happens to name a tool `run_office_js` unable to obtain
auto-approval (spoof-safe); the exact-string match makes `"READ"`/`"read "`/junk fail
safe (TEST-10 pins every case).

### DEC-3: Where does the classifier live?
**Resolution:** A new server module `src-app/server/src/modules/mcp/chat_extension/office_approval.rs`
holding `office_bridge_mcp_server_id()`, `run_office_js_read_bypass(...)`, the extracted
`compute_needs_approval(...)`, and their `#[cfg(test)]` unit tests. `mcp.rs` calls it.
**Basis:** convention — mirrors `control_mcp/handlers.rs` (classifier + pure decision +
in-source tests, called from the approval loop).

### DEC-4: Does the daemon (`dispatch_tool`) read or validate `mode`?
**Resolution:** No. `mode` is consumed ONLY by the server approval loop. The desktop
daemon and the pane ignore it entirely — execution of a `run_office_js` script is
byte-identical regardless of `mode`. There is NO pane-side read-only enforcement
(no Proxy). The `run_office_js` dispatch arm is unchanged by this feature.
**Basis:** user — explicit decision to trust the declared mode rather than build the
enforced read-only Proxy ("that's a lot to implement then, let LLM decide it").

### DEC-5: Signature of the extracted decision, and behavior-preservation.
**Resolution:** `compute_needs_approval(server_id: Uuid, tool_name: &str, input: &Value,
approval_mode: ApprovalMode, is_builtin: bool, is_control: bool, auto_approved: bool)
-> bool` — returns the `needs_approval` boolean, reproducing the current `mcp.rs:2120`
if/else EXACTLY (control → `control_call_needs_approval`; builtin → false; else per
`ApprovalMode`, honoring the precomputed `auto_approved`), with ONE added arm before
the `else`: office_bridge `run_office_js` `read` → false. The Disabled-mode → deny
early-continue at `mcp.rs:2103` is left in place unchanged (it precedes this decision).
`auto_approved` is passed in precomputed (the existing `auto_approved_servers`/
`user_auto_approved` `contains_tool` check) so the pure function stays DB-free.
**Basis:** codebase — minimal behavior-preserving extraction; TEST-12 pins all 9 branches.

### DEC-6: How does the server get office_bridge's id without depending on the desktop crate?
**Resolution:** `office_bridge_mcp_server_id()` recomputes the same deterministic id the
desktop registers: `Uuid::new_v5(&Uuid::NAMESPACE_URL, b"office_bridge.ziee.internal")`.
A desktop-crate test (`mod.rs`, TEST-11) asserts the desktop `office_bridge` row id ==
`ziee::…::office_bridge_mcp_server_id()` so the two derivations can never drift (the
desktop crate depends on the server lib, so it sees both).
**Basis:** codebase — deterministic v5 ids are already the module convention
(`office_bridge.ziee.internal` is the exact string the desktop `mod.rs` uses).

### DEC-7: What backs the real-LLM end-to-end test (TEST-17)?
**Resolution:** The OpenAI-compatible LiteLLM proxy on `coder.ziee:4000`
(`qwen3.6-35b-a3b`) via the SSH tunnel, env-gated on `ZIEE_OFFICE_REAL_LLM_URL` with a
soft-skip when unset. It uses a MOCK office MCP server (registered under
`office_bridge_mcp_server_id()` exposing `run_office_js`), so it needs NO live Excel
pane — it verifies only that a real model picks `mode` correctly and the approval loop
gates accordingly.
**Basis:** user (prior instruction to use the `coder.ziee` endpoint) + codebase
(soft-skip pattern from `injection_test`).

### DEC-8: Recorded accepted security trade-offs (trust-based model).
**Resolution:** Shipping the trust-the-declared-mode model WITHOUT read-only
enforcement is a conscious choice with three accepted risks, documented in
`OFFICE_TOOL_SURFACE_DESIGN.md`: (1) a prompt-injected `mode:"read"` script that
actually mutates bypasses approval (no enforcement); (2) auto-approved reads are a
silent full-document-content exfiltration channel (auditable via `mcp_tool_calls`, not
blocked); (3) "always allow" grants every subsequent write in the conversation,
including a later injected one. The enforced read-only Proxy remains a documented
future option if the threat model tightens.
**Basis:** user — explicitly weighed and accepted after the read-op/enforcement analysis.
