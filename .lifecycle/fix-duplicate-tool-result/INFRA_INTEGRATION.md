# INFRA_INTEGRATION — fix-duplicate-tool-result

The three mandatory Phase-5 walks. This is a backend correctness fix with no UI
surface, so the "user" in the UX walk is the person chatting and the model consuming
the request.

## 1. User-experience walk

**How a real user hits this today.** They ask something that makes the model fire a
parallel batch mixing an approval-exempt built-in (`remember`, `web_search`,
`read_file`) with an approval-required external MCP tool (an RCPA DE analysis). ziee
shows the approval prompt; they click Approve — the natural, encouraged action. The
turn then dies with a raw provider error: *"AI provider error: Invalid request:
messages.14.content.2: each tool_use must have a single result…"*. There is no
recovery affordance: re-sending re-assembles the same stored history and fails
identically, so the CONVERSATION IS BRICKED. That is why this is worth the fix, not
just the error text.

**After the fix.** Approve → the placeholder is upgraded in place to the real result
→ the request is valid → the turn continues and the model sees the actual DE-analysis
output. No new UI, no new copy, nothing for the user to learn. The failure simply
stops happening.

**What the user does NOT get:** already-bricked conversations are not repaired. The
fix is in ASSEMBLY (what we send), and stored history is untouched — but that is
sufficient, because assembly is re-run from stored history on every send. A
previously-failing conversation starts working again on the next send. Verified by
reasoning about the data flow AND pinned by TEST-1, which assembles from exactly the
stored-block shape a bricked conversation holds.

## 2. Infrastructure-integration walk

Every subsystem the diff touches, and what each demanded:

| Subsystem | Constraint found | How it is handled |
|---|---|---|
| **Chat streaming pipeline** | `history` is snapshotted ONCE at `streaming.rs:197`, before the agentic loop; each iteration re-assembles from that snapshot. So a result persisted mid-loop is NOT in the snapshot — which is precisely why `before_llm_call` must also mutate `request.messages` (persist + push are both required, not redundant). | ITEM-1 keeps BOTH (persist for the next turn, fold into the request for this one) and only changes HOW the fold happens. |
| **MCP approval flow** | The approval-resume path re-sends and continues the SAME `assistant_message_id`, and `before_llm_call` both persists results AND pushes them onto the request. The pure awaiting-approval batch (no result yet) deliberately gets a bare Assistant turn whose pairing is completed by the pushed User message. | ITEM-1's leftover-vec return preserves that path exactly (TEST-7 pins it). Breaking it would re-open the sibling `chat-toolresult-pairing` 400. |
| **Built-in MCP servers** | Built-ins are approval-BYPASSED (`is_builtin_server_id`), which is exactly what creates the mixed batch: a built-in resolves while an external tool waits. Not a bug in the bypass — the bypass is correct; the assembly assumption about it was wrong. | Root cause understood as an assembly defect, not an approval-policy change. No bypass semantics touched. |
| **Context trimming** | `clear_old_tool_results` keep-last-K is a COUNT over `tool_result` positions, so it must count the deduped set or the window is computed against phantom blocks. | ITEM-2 runs BEFORE it (DEC-3). `mod trim_tests` re-run green as the guard. |
| **Summarization** | `apply_summary_block` cuts the outbound array by DB-message count and snaps past a leading orphan Tool message (`summarizer.rs:414`, from the sibling fix). It runs on the same `chat_request.messages`. | ITEM-2 can only REMOVE a duplicate block/message, never reorder, so the snap-forward logic is unaffected. A message emptied by dedup is removed rather than left as an empty husk the snap would have to reason about. |
| **`get_tool_result` recall** | Reads stored history `ORDER BY created_at DESC LIMIT 1` (keep-LAST), while history reconstruction is keep-FIRST. If duplicate ROWS exist the model sees different content via the two paths. | ITEM-4 removes the mechanism that created duplicate ROWS (re-execution), so the two paths converge. The inconsistency itself is pre-existing and out of scope — noted, not silently "fixed". |
| **`mcp_tool_calls` history** | Recording happens inside `McpSession::call_tool`, fire-and-forget. A re-executed tool would record TWICE. | ITEM-4's exactly-once execution fixes that as a side effect; TEST-10 asserts one `tools/call` reaches the mock. |
| **Sync / SSE** | No new entity, no emit-site change. | Nothing to do — confirmed by grep, not assumed. |
| **Permissions** | No new permission; no gating change. | A9/A10 do not apply (recorded in BASE.md). |
| **Workflow runner** | `persist_links` / the workflow tool dispatcher call into MCP but not into `execute_approved_tools_sync` or the chat request assembler. | Out of the blast radius — verified by call-graph grep. |

**The find this walk produced.** Auditing the approval loop for ITEM-4 surfaced that
the row was deleted in **four** places (the post-execution site plus three error arms
that each `continue`d before it). With the claim hoisted to the top of the loop body,
all three arm-deletes became dead double-deletes. Removed them: the loop now has ONE
claim point. This was not in the plan — recorded as DRIFT-1.1 (impl-wins) and it makes
the anti-spin property structural rather than something each new error arm must
remember to re-implement.

## 3. Entity-lifecycle walk

Entities held by the surfaces this diff touches, and the add/remove/delete/mutate/
access-loss paths for each:

- **`tool_use_approvals` row.** Created at pause; **claimed (deleted)** at the top of
  execution (ITEM-4); deleted on denial (`mcp.rs`, denied path — untouched); cascade-
  deleted with its message/branch. Access-loss: if the user loses access to the MCP
  server between pause and approve, `get_all_accessible_config` no longer returns it →
  the "server not found" arm fires → an `is_error` result is recorded and the (already
  claimed) row does not resurrect. **Ran it:** TEST-10 asserts the row is gone after
  execution; the pre-existing `mcp_approval_loop_unresolvable_tool_errors_and_terminates`
  exercises the unresolvable arm and proves no spin — that test now depends entirely on
  my claim doing the delete, since I removed that arm's own delete. Both pass.
- **`message_contents` `tool_result` row.** Appended by the approval path; never
  updated; cascade-deleted with the message. Duplicate-row creation is what ITEM-4
  removes. **Ran it:** TEST-10 asserts exactly one row per `tool_use_id`.
- **`tool_result` BLOCK in the assembled request (transient).** Created by
  `group_assistant_blocks` (real or synthesized); **mutated in place** by ITEM-1;
  content-cleared/truncated by `clear_old_tool_results`; dropped by ITEM-2 if
  duplicated. Lives only for one request — no persistence, no sync, nothing to revoke.
- **The `message_contents` unique index (ITEM-6).** Dropped by migration 158 on every
  existing DB. **Ran it:** TEST-11 asserts the surviving constraint still REJECTS a
  colliding slot, so the drop removed the duplicate and not the protection.
