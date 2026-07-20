# DECISIONS — fix conversation titles under `manual_approve`

Every human/product input the implementation needs, resolved up front. Genuine product choices were
escalated to the lead as `AskUserQuestion` pickers (DEC-1, DEC-2, DEC-9); the rest resolve by codebase
convention with the precedent named.

### DEC-1: Is item 2 (gpt-oss tool-name routing) in this PR, or its own?
**Resolution:** In this PR — diagnose live on the review container and fix. If the diagnosis reveals a
large cause, or one that is model-side rather than a ziee bug, ITEM-5 splits to its own follow-up PR
and this PR ships the rest (recorded then as an approved DESCOPE below).
**Basis:** user — lead chose "Investigate and fix" over the recommended defer, and directed copying the
GPT-OSS provider + BioGnosia MCP server to the review container to test. Split gate added by the lead.

### DEC-2: Is item 4 (untitled display label) in this PR?
**Resolution:** Yes, including the backend `first_message_preview` field it turned out to require.
**Basis:** user — lead chose "Bundle it in", then, when phase-2 exploration proved the sidebar has no
message text client-side, chose "Add first_message_preview to the list response" over deferring or
shipping a degraded frontend-only version.

### DEC-3: Re-call `call_after_llm_call`, or add a dedicated hook?
**Resolution:** A dedicated `after_llm_skipped` trait hook with a default no-op impl, plus
`ExtensionRegistry::call_after_llm_skipped`.
**Basis:** codebase — re-calling `call_after_llm_call` re-enters MCP's `after_llm_call`, whose STEP 1
(`mcp.rs:2362-2406`) unconditionally executes `get_approved_tools_for_branch`. On the
"approvals still pending" path (`mcp.rs:1613`) approved-but-unexecuted rows exist, so a re-call would
execute approved tools EARLY, out of band from the resume request. A dedicated hook has no such
re-entry.

### DEC-4: Which extensions implement the new hook?
**Resolution:** Only `title`. All others keep the default no-op.
**Basis:** codebase — the bug is title-specific; every other extension either already ran its work in
`before_llm_call` on these paths (MCP) or has no post-turn duty. A default impl keeps all 19 existing
`impl ChatExtension` blocks untouched.

### DEC-5: Does the new fan-out abort on an extension error, like `call_after_llm_call` does?
**Resolution:** No — it logs and continues, and the call itself returns `Ok`.
**Basis:** convention + rationale — `call_after_llm_call` uses `?` (`registry.rs:401-423`), but it runs
inside `finalize()` where the caller already defaults to `Complete` on error (`streaming.rs:1588-1594`).
On the skipped path the user's answer is ALREADY persisted and streamed, so a title-generation failure
must not fail the turn. The deviation is documented inline at the definition.

### DEC-6: Where exactly does the hook fire, relative to the terminal chunk?
**Resolution:** BEFORE the terminal chunk in both arms. `Complete` arm: hook → `extension_complete`
chunk. `CompleteWithContent` arm: `append_content` → text chunk → hook → `stop` chunk.
**Basis:** codebase — `start_generation` drains `ext_rx` at its tail (`streaming.rs:953-964`) on the
invariant that extension events precede the terminal chunk; an event emitted after it is dropped. The
hook must also follow `append_content` so `assistant_produced_output` (`title.rs:91-99`) sees the answer
text.

### DEC-7: Do the error breaks and the failsafe `max_iterations` break also get the hook?
**Resolution:** No. `streaming.rs:354`, `:508` (error breaks) and `:233` (failsafe cap) are untouched.
**Basis:** convention — a FAILED turn should not be titled, and the failsafe path already ran
`finalize()` (and therefore `after_llm_call`) on its prior iteration. Recorded as a deliberate non-goal
in PLAN.md so phase 6 does not read it as an omission.

### DEC-8: Which branch does `first_message_preview` read from?
**Resolution:** The conversation's ACTIVE branch (`c.active_branch_id`), first `text` content of the
first user message.
**Basis:** convention — identical scoping to the search `EXISTS` subquery in the SAME query
(`repository/conversations.rs:171-181`), which documents why: content in a superseded edit branch is
invisible when the conversation is opened. Using a different scope for the preview than for search would
be internally inconsistent.

### DEC-9: Is the preview truncation cap a fixed constant or an admin-configurable setting?
**Resolution:** A fixed named constant, `CONVERSATION_PREVIEW_MAX_CHARS = 120`, defined next to the
query — NOT an admin-configurable settings row.
**Basis:** convention + explicit rationale for the exception. The lifecycle's configurable-settings rule
defaults operational tunables to admin-configurable, but this is a **display-string truncation**, not an
operational tunable: it has no resource, retention, quota, or security dimension, and no operator would
tune it. The closest precedent is `TITLE_MAX_CHARS` in `title.rs`, likewise a fixed constant for a
display string. It is a named constant (not an inline magic number) so it can be promoted later without
a rewrite. 120 chars comfortably fills the widest sidebar row while keeping the list payload small.

### DEC-10: Does the preview truncate server-side or client-side?
**Resolution:** Server-side, in the query projection.
**Basis:** convention — the sidebar list is paginated and can hold long messages; shipping full message
bodies for every row would bloat the response for a string that is visually truncated anyway. Matches
the repo's existing practice of capping payloads at the boundary (e.g. the MCP tool-call result caps).

### DEC-11: Does the derived label get written back as a real title anywhere?
**Resolution:** Never. The DB `title` column stays null; the label is display-only. `TitleEditor`
continues to edit and save the real `title` field and must not prefill from the preview.
**Basis:** user + codebase — PR #165 deliberately deleted the fallback that persisted the user's raw
message as a permanent title. Re-introducing it via the editor would undo that fix. TEST-15 asserts the
contract.

### DEC-12: Which of the 9 fallback sites adopt the shared helper?
**Resolution:** All 9, including `PaneManagerDrawer.tsx:75` (which today uses a DIFFERENT
`|| 'Conversation'` literal) and `RecentConversationsWidget.tsx:374` (the delete-confirm dialog copy).
**Basis:** convention — the delete-confirm SHOULD name the conversation the way the sidebar does, or the
user cannot tell which row they are deleting; that is the same job-to-be-done the item exists for.
Consolidating all 9 removes the two divergent literals.

### DEC-19: Does the `TitleEditor` HEADER show the derived preview? (amends DEC-12)
**Resolution:** No. `TitleEditor.tsx:154` keeps the placeholder — routed through the shared
`UNTITLED_CONVERSATION_LABEL` constant so the literal is not duplicated, but WITHOUT the preview
fallback. The other 8 sites (lists, pickers, delete-confirm) use the full `conversationDisplayLabel`.
**Basis:** codebase + rationale, discovered during implementation. Two independent reasons:
(1) `TitleEditor` binds to `Stores.Chat.conversation`, which is a `Conversation` (the DB row model
returned by `GET /conversations/{id}`), NOT the `ConversationResponse` the list endpoints return — it
has no `first_message_preview`, and adding one would mean putting a non-column on the row model.
(2) More importantly it is the wrong UX: the header IS the edit affordance, so showing a derived label
there would tell the user a title exists when none does, and invite them to "edit" a value that is not
persisted. "Untitled Conversation" is the honest label at the one place you go to fix it. The
job-to-be-done — telling N sidebar rows apart — is entirely served by the list surfaces.
This is a deliberate scope decision, not an omission; phase 6 should read it as such.

### DEC-13: What is the diff base for every lifecycle gate?
**Resolution:** `origin/khoi`, passed explicitly as `--base origin/khoi`.
**Basis:** codebase — the validator defaults to `origin/main` (`lifecycle-check.mjs:122-124`), but this
branch is cut from `khoi`. The default would pull PR #165's 7 files into the phase-6 coverage law,
demanding audit angles on code this PR did not write.

### DEC-14: Which port does the review container use?
**Resolution:** A port confirmed free at build time, explicitly NOT 8080, 18131, 18132, 18133, or
18134. The chosen port is recorded in the final report.
**Basis:** user — the lead reserved those five (8080 is the running production-like stack this PR copies
data FROM; the 1813x range is other reviewers' stacks).

### DEC-15: How is BioGnosia registered on the review container?
**Resolution:** As a SYSTEM MCP server — `is_system=true`, `is_built_in=false`. Not a user MCP server.
**Basis:** user — explicit instruction from the lead.

### DEC-16: What prompt drives the repro?
**Resolution:** A plain systems-biology question chosen to make the model call `query_rag` and NOTHING
else; the run is invalid unless verification confirms `query_rag` was the only tool that fired.
**Basis:** user — the lead's critical test-design constraint. A turn that also fires `web_search` or
`code_sandbox` takes a different code path and would not exercise the single-tool `audience:["user"]`
bypass that is the actual bug.

### DEC-17: Are the three test-debt cleanups (item 6's "cheap cleanups") in scope?
**Resolution:** Yes — all three, as ITEM-9. The 101 pre-existing test failures themselves are NOT.
**Basis:** user — the brief permits bundling them "ONLY if trivial"; all three are single-line test-file
edits enabled by PR #165's `is_title_request` seam. The failures are explicitly out of scope per the
brief.

### DEC-18: Where does the new integration test live?
**Resolution:** A new file, `tests/chat/title_approval_test.rs`, rather than appended to
`title_test.rs`.
**Basis:** convention — `title_test.rs` uses `common::stub_chat` (plan-scripted), while the approval
flow needs `oai_capture_stub` + `MockMcpServer`, which is exactly how `title_audience_test.rs` is
already split out as its own file. A third file for the approval variant follows that established split.

### DEC-20: What does the live gpt-oss diagnosis conclude? (resolves ITEM-4/5/6)
**Resolution:** There is **no ziee defect to fix**. ITEM-4 ships as a diagnostics improvement only;
**ITEM-5 and ITEM-6 are DESCOPED** under DEC-1's pre-approved split gate.
**Basis:** live evidence from the review container (`:18140`, real gpt-oss-120b + real BioGnosia
registered as a system MCP server), plus code archaeology:

1. **The model behavior IS real and was reproduced.** gpt-oss-120b emitted the tool name WITHOUT the
   `<server_id>__` prefix — bare `query_rag`, exactly as reported.
2. **ziee already handles it.** The run logged
   `[mcp] Recovered server_id for prefix-less tool name 'query_rag' -> 'query_rag': 8a5d68f7-…`;
   the tool routed, executed, returned, and the turn terminated normally with a title.
   `grep -c 'no valid server_id prefix'` over the whole container log = **0**.
3. **The fix the brief proposed already exists**, landed in **b5a4fa7e8 (2026-07-10)** —
   `resolve_server_and_tool` (`mcp.rs:354-373`) — which is an ancestor of this branch.
4. **H1 disproved for `query_rag`:** probing all three user MCP servers on the live `:8080` deployment
   shows `query_rag` is advertised by BioGnosia ALONE, so it is uniquely recoverable.
5. **H2 disproved:** the two early `Continue` returns fire only when no tools are advertised, so the
   model would have nothing to call.
6. **No stall mechanism remains:** unroutable tool_uses already get synthetic error `tool_result`s
   (`mcp.rs:2894-2918`, `:2922-2943`, `:930-952`) and `max_iteration` defaults to **10**, not unlimited.
7. **The original evidence is unavailable:** the prior worker's conversations lived on the
   `ziee-review-title` stack at `:18133`, which no longer runs.

Shipping ITEM-6 anyway would change agentic-loop termination for EVERY model in order to fix a stall
that cannot be reproduced and whose mechanism is bounded — speculative risk with no confirmed defect.
The one genuine latent case found (`validate_input_file` IS advertised by both RCPA `:9004` and DSCC
`:9006`, so it is correctly marked ambiguous and refused) is arguably CORRECT behavior: auto-resolving
it could mis-dispatch a side-effecting tool to the wrong server.

---

## Descope dispositions

- DESCOPED: ITEM-5 — the live repro shows gpt-oss prefix-less names ALREADY resolve correctly (recovery landed in b5a4fa7e8); there is no reproducible defect to fix, and the reported stall mechanism is bounded by max_iteration=10 [approved: khoi — split gate pre-approved 2026-07-20, see DEC-1]
- DESCOPED: ITEM-6 — a repeated-unroutable-call terminator would change agentic-loop termination for ALL models to fix an unreproducible stall; unroutable calls already emit synthetic error tool_results and the loop is already capped [approved: khoi — split gate pre-approved 2026-07-20, see DEC-1]

Both are recorded decisions backed by live evidence (DEC-20), not silent cuts. ITEM-4 still ships: the
warn site now names the advertised tool set and its ambiguity state, so the next live report is
diagnosable in one run instead of three hypotheses.
