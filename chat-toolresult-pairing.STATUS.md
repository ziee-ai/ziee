# STATUS — chat-toolresult-pairing

**Owner:** khoi worker · **Branch:** `feat/chat-toolresult-pairing` (off `origin/khoi`)
**Worktree:** `/data/khoi/home-workspace/ziee/tmp/chat-toolresult-pairing-wt`

## Scope
Make the conversation→provider-request assembler ALWAYS emit a valid Anthropic/OpenAI/Gemini
sequence (every `tool_use` answered by a `tool_result` in the next message), regardless of
whether tools succeed or fail. Backend-only.

## Root cause (two independent defects, both fixed)
1. `group_assistant_blocks` (streaming.rs) dropped accumulated `tool_result`s in its trailing
   branch when a co-located parallel batch wasn't fully resolved → dangling `tool_use` → 400.
2. `apply_summary_block` (summarizer.rs) drained the outbound array by DB-message count, but
   the array was already split into Assistant/Tool pairs → the cut could land between a
   `tool_use` and its result.

## Fix
- `group_assistant_blocks`: single `flush_assistant_tool_pair` helper answers every `tool_use`
  with its real result or a synthesized `is_error` placeholder (carrying the tool_use name for
  Gemini); orphan results dropped. A no-result-yet (approval/in-progress) batch still emits a
  lone Assistant turn so its separately-appended result isn't raced.
- `apply_summary_block`: snap the drain boundary forward past any leading orphan `Role::Tool`.

## Coordination with `stale-artifact-links`
That worker fixes the CAUSE (why the RCPA/DSCC file fetch fails → "Failed to fetch"). MY fix
makes the payload valid regardless of that outcome. **Different files** (they touch
`code_sandbox/tools/files.rs` + resource-link path; I touch chat/summarization assembler) —
no collision expected. A failed tool still yields a valid `is_error` `tool_result`, so their
failure-text choice and my pairing fix compose cleanly.

## Progress — COMPLETE (lifecycle 9/9), PR open vs `khoi`
- [x] Phases 1–9 green (`lifecycle-check --all` = 9/9).
- [x] Unit tests (streaming + summarizer) green (74/74); integration `assistant_block_grouping`
      green (5/5, incl. preserved approval-flow test); non-real-LLM summarization suite 23/23.
- [x] Blind multi-angle audit (10 angles × 3 rounds, 7 fresh diff-only auditors) → converged
      to 0 new confirmed findings. Provider-contract auditor confirmed the synthesized
      `is_error` result is valid for Anthropic (id), OpenAI (tool_call_id), Gemini (name).
- [x] **Live container repro (fixed binary on :18099 + real vLLM gpt-oss-120b + `fetch` MCP):**
      induced the exact co-located `2×tool_use + 2×tool_result("Failed to fetch")` assistant
      message, then sent the next turn → **no 400, no "tool_use without tool_result"** in the
      server log; normal response. Live 8080 / RCPA / DSCC left untouched; server torn down.
- [x] `.lifecycle/` stripped in the final commit; PR opened against `khoi` (not merged).
