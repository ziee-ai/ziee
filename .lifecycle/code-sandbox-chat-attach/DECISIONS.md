# DECISIONS — code-sandbox-chat-attach

All decisions resolved up front. No product/human choice is open: the ATTACH
POLICY (auto-attach-when-enabled) and the APPROVAL POSTURE (stay approval-gated)
were the two genuine product choices and both were decided in the task brief /
diagnosis; everything else is resolved by codebase convention.

### DEC-1: Set the attach flag via a dedicated `code_sandbox/chat_extension/` submodule, or inside the existing `mount_context_extension.rs`?
**Resolution:** A dedicated `code_sandbox/chat_extension/` submodule mirroring `web_search/chat_extension/`.
**Basis:** convention — every other built-in MCP server (web_search, lit_search, citations, bio, control, skill, js_tool, knowledge_base) owns a dedicated `chat_extension/` module with a shared `ATTACH_FLAG` const; code_sandbox is the anomaly. A dedicated extension keeps the flag key readable by mcp.rs as `crate::modules::code_sandbox::chat_extension::ATTACH_FLAG` (identical to the 7 siblings), and keeps the mounts extension's single concern intact. The mounts extension also early-returns on `!supports_extra_mounts()`, which is a MOUNTS gate that must NOT gate the tool attach — a separate extension avoids entangling the two.

### DEC-2: Prepend a system "nudge" message about `execute_command` (as web_search does)?
**Resolution:** No nudge. The extension only sets the attach flag.
**Basis:** convention/codebase — the `execute_command` tool description already teaches usage; the existing `code_sandbox_mounts` extension already injects mount context; and `spawn_background`'s always-on description already references `execute_command`. The bug is purely non-advertisement, so attaching the tool is the whole fix. A nudge would be scope creep. (web_search adds a nudge because it also carries an untrusted-content safety rule for fetched pages; code_sandbox has no such per-turn safety string to inject.)

### DEC-3: Attach policy — when is code_sandbox advertised to a chat?
**Resolution:** Auto-attach when code_sandbox is enabled AND the model is tool-capable (mirrors web_search / background). No per-user opt-in, no per-conversation toggle.
**Basis:** user — chosen in the task brief/diagnosis ("chosen policy: AUTO-ATTACH WHEN ENABLED"). Consistent with how execution-adjacent always-on tools (`spawn_background`) attach for every tool-capable model.

### DEC-4: Approval posture — is `execute_command` approval-bypassed once attached?
**Resolution:** No. code_sandbox is deliberately NOT added to `is_builtin_server_id` / `builtin_server_ids()`; `execute_command` stays behind manual approval.
**Basis:** user + convention — the brief mandates it, and it matches the established posture for execution/mutation subsystems (`code_sandbox`, `workflow`, `background`, `control` are all attached-but-not-bypassed). Pinned by TEST-4 + the pre-existing `all_readonly_builtins_share_approval_bypass_but_execution_ones_do_not`.

### DEC-5: Chat-extension registration order.
**Resolution:** Order 21 (< 30, before the MCP collector).
**Basis:** convention — the flag must be set before `auto_attach_builtin_ids` runs in the MCP extension (order 30). Siblings sit at 20-29 (file 20, control 22, js_tool/knowledge_base 23, summarization 24, memory 25, web_search 26, bio 27, lit_search 28, citations 29). 21 is a free slot in that band and has no ordering dependency on the mounts extension (order 12) — both key off `get_state()` independently.

### DEC-6: Enabled predicate for the gate.
**Resolution:** `code_sandbox::config::get_state().is_some()` (true only when enabled + init reached Ready).
**Basis:** convention/codebase — the same predicate `mount_context_extension.rs` and `client/stdio.rs` use to decide whether the sandbox is live. `get_state()` is `None` when `code_sandbox.enabled: false` or before boot, so a disabled deployment never sets the flag.

### DEC-7: Is any NEW operational tunable introduced (Phase-4 configurable-settings rule)?
**Resolution:** No new tunable. The only on/off control is the EXISTING `code_sandbox.enabled` deploy-level config kill-switch (already the module's enable gate). No settings table, migration, or admin card is added.
**Basis:** convention — the attach behavior is a fixed consequence of the enabled state, not a separately tunable knob; adding one would be gratuitous. The existing kill-switch already governs the whole subsystem (and thus attachment).

### DEC-8: (fix round 1) Does `execute_command` force approval even under `ApprovalMode::AutoApprove`?
**Resolution:** Yes. Add an `is_code_sandbox` force-approval arm so `execute_command` (and the mutating `write_file` / `edit_file`) always require explicit approval regardless of the conversation's approval mode.
**Basis:** user + convention — the brief mandates `execute_command` "MUST stay behind manual approval"; auto-attaching it otherwise lets arbitrary code auto-run under AutoApprove. `control` (`invoke_capability`) and `background` (`spawn_background`) already do exactly this (force-approval overriding AutoApprove), so this is the established posture for auto-attached-but-dangerous built-ins. Surfaced by the blind security audit (LEDGER round 1).

### DEC-9: (fix round 1) Which code_sandbox tools force approval vs auto-run?
**Resolution:** Force approval: `execute_command` (arbitrary code), `write_file`, `edit_file` (workspace mutations), and any unrecognized tool (fail-safe). Auto-run: `read_file`, `list_files`, `get_resource_link` (read-only, scoped to the conversation's own sandbox workspace).
**Basis:** convention — mirrors `background_call_needs_approval`'s read-vs-write split (reads auto-run, writes/launches + unknown force approval). The reads only touch the per-conversation isolated workspace, so auto-running them under AutoApprove is safe; execution + mutation are the sensitive ops.
