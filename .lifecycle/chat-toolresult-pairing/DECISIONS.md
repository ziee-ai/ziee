# DECISIONS — all resolved up front

### DEC-1: When a `tool_use` has no matching `tool_result`, synthesize a result or drop the tool_use?
**Resolution:** Synthesize an `is_error` `ToolResult` for each unresolved id; never drop the tool_use — BUT only for a batch whose tools have actually run (a completed batch, or a completed-but-partial trailing batch with ≥1 captured result). A trailing batch with NO result captured yet (in-progress / awaiting-approval) is left as a single Assistant turn, because its real result is appended separately by the approval-resume path and synthesizing would race it (see DRIFT-1.1).
**Basis:** user + codebase — the task file mandates "match every tool_use id" and "a failed tool must STILL produce a tool_result" (which applies once the tool has run and produced/failed a result); the approval-flow exception is required by the existing `trailing_tool_use_without_result_is_emitted_as_assistant` contract.

### DEC-2: What identifier/name does the synthesized `ToolResult` carry?
**Resolution:** `tool_use_id` = the unresolved tool_use's `id`; `name` = the tool_use's `name` (the `server_id__tool` prefixed form as it sits in the assembled block); `content` = a single `Text` block; `is_error: Some(true)`.
**Basis:** codebase — Anthropic/OpenAI pair by id (`tool_use_id`/`tool_call_id`), Gemini pairs `functionResponse` by name and falls back to `"unknown_function"` with a warning when name is absent (`gemini.rs:358-362`); carrying the tool_use's own name is correct for all three adapters.

### DEC-3: Wording of the synthesized error result text?
**Resolution:** `"Tool result unavailable (no result was recorded for this tool call)."`
**Basis:** convention — mirrors the short, model-actionable synthetic-error strings already used on MCP failure paths (`mcp.rs`, e.g. "Tool execution stopped: maximum iteration limit reached."). No secrets, no ids leaked beyond the already-present tool_use_id.

### DEC-4: Orphan trailing `tool_result` with no matching `tool_use` — keep or drop?
**Resolution:** Drop it (emit no Tool turn for it).
**Basis:** codebase + provider requirement — a `tool_result` with no preceding assistant `tool_use` is itself a provider 400; `group_assistant_blocks` already drops a lone orphan result today, so this is existing behavior, not a change.

### DEC-5: Summarizer boundary — snap the cut forward, or recount `message_count` in outbound-message space?
**Resolution:** Snap `drop_until` forward past any leading `Role::Tool` message(s) in the retained set; do NOT recount.
**Basis:** codebase — `group_assistant_blocks` always emits a Tool turn immediately after its Assistant tool_use turn, so a retained-leading `Role::Tool` is provably an orphan whose tool_use is in the dropped prefix. Snapping is a 1-line local change; recounting would couple the summarizer to the split-message model and risk new off-by-one bugs.

### DEC-6: Is any new operational tunable / config settings row introduced?
**Resolution:** No. No resource limit, retention period, threshold, or toggle is added. Existing constants (`CLEAR_TOOL_RESULTS_TOKEN_THRESHOLD`, `KEEP_LAST_TOOL_RESULTS`, `MAX_KEPT_TOOL_RESULT_CHARS`, `summary.message_count`) are unchanged.
**Basis:** convention — the configurable-settings rule applies only when a feature introduces a tunable; this fix introduces none, so no settings table/migration/REST/sync/admin-card is warranted.

### DEC-7: Does the fix belong in the assembler layer or the provider adapters?
**Resolution:** Assembler + summarizer layer only (`group_assistant_blocks`, `apply_summary_block`); adapters untouched.
**Basis:** codebase — the three adapters are correct 1:1 per-message mappers that assume pre-paired input; fixing at the layer that produces the message list keeps Anthropic/OpenAI/Gemini simultaneously correct and avoids per-adapter duplication.
