# INFRA_INTEGRATION — the phase-5 walks

## User-experience walk

**ITEM-1/2/3 (the title fix).** A real user opens a BioGnosia chat under
`manual_approve`, asks a question, is shown an approval prompt, clicks Approve, reads the answer — and
the sidebar row STILL says "Untitled Conversation". Nothing tells them why, and nothing they can do
fixes it; only sending a second message does. After the fix the title lands as the answer arrives, via
the `TitleUpdated` SSE event, with no reload.

**ITEM-7/8/10 (the label).** A user whose title provider is misconfigured sees N identical "Untitled
Conversation" rows and cannot tell them apart; the search box only matches the literal word "Untitled".
After the change each row shows what they actually asked, and search matches that text.

## Infrastructure-integration walk

Every subsystem the diff touches, and what it demanded:

| Subsystem | Constraint found | Handled |
|---|---|---|
| **Streaming loop** | The two `BeforeLlmAction` break arms end the spawned task outright — there is no post-loop code (`streaming.rs:769-774` is a comment). | Hook called inside each arm. |
| **SSE / extension channel** | `start_generation` drains `ext_rx` at its tail on the invariant that extension events precede the terminal chunk (`streaming.rs:953-964`). A hook after the terminal chunk would silently DROP `TitleUpdated`. | Hook precedes the terminal chunk in both arms; asserted by TEST-6. |
| **MCP approval flow** | MCP's `after_llm_call` STEP 1 (`mcp.rs:2362-2406`) executes `get_approved_tools_for_branch` unconditionally. Re-calling the existing fan-out would execute approved tools EARLY on the "approvals still pending" path. | Dedicated hook, implemented only by title; MCP keeps the default no-op. |
| **Title extension gating** | `assistant_produced_output` (`title.rs:91-99`) needs the answer already on the assistant row. | Hook ordered AFTER the `append_content` in the `CompleteWithContent` arm. |
| **Extension registry** | `call_after_llm_call` aborts the fan-out on `?`; acceptable there because `finalize`'s caller degrades to `Complete`. The new hook has no such caller. | New fan-out logs and swallows; TEST-3. |
| **Chat projects** | `ProjectConversationsList` reuses the SAME `ConversationCard` (`ProjectConversationsList.tsx:6,84`). Populating the preview only on `/conversations` would make project rows show "Untitled" while the sidebar showed a preview. | The project list query got the same projection (`project/chat_extension/repository.rs`). |
| **Branching / edits** | A superseded edit branch's content is invisible when the conversation is opened. | Preview scoped to `c.active_branch_id`, matching the existing search `EXISTS` subquery's documented rationale. |
| **Pagination / scale** | The sidebar list is paginated + virtualized; a per-row fetch would be N+1. | Preview rides the existing list response as one LATERAL-joined column, truncated server-side to 120 chars. |
| **Aggregates** | The list query `GROUP BY c.id` and orders by `COUNT(bm.message_id)`; a fan-out join would inflate `message_count`. | `LEFT JOIN LATERAL (… LIMIT 1)` yields exactly one row per conversation; `fm.text` added to the GROUP BY (Postgres infers functional dependency only for columns of `c`). Asserted by TEST-16. |
| **OpenAPI / desktop** | `types_ts_parity` is a byte-for-byte golden test; the desktop binary emits its own spec. | `--generate-openapi` run for BOTH binaries; 4 generated files committed. |
| **Permissions** | No new permission. The preview rides the already-gated `GET /conversations` (owner-scoped). | No A10 `[negative-perm]` spec required. |
| **Sync** | No new sync entity: the preview is a field on an existing response, refetched by the existing `sync:conversation` subscription. | Nothing to add. |

## Entity-lifecycle walk (ITEM-7/10)

The only entity the change touches is the **conversation row as rendered in a list**.

- **ADD** — a new conversation has no messages → `first_message_preview` is `NULL` → the helper falls
  through to "Untitled Conversation". Asserted by TEST-16.
- **MUTATE (title set later)** — the preview is populated INDEPENDENTLY of the title; the client applies
  precedence. So a conversation titled on turn 1 immediately renders its real title, and one that loses
  its title still has a label. Coupling them server-side would have made a lost title unrecoverable.
- **MUTATE (first message edited)** — an edit creates a new branch; the preview follows
  `active_branch_id`, so it tracks what the user currently sees.
- **REMOVE / access-loss** — unchanged: the field is part of an existing owner-scoped response, so a
  conversation the user cannot read returns nothing at all, preview included.
- **SYNC vs LOCAL** — no new handler. The preview arrives on the same refetch both paths already use, so
  there is no second code path that could miss it.

## ITEM-4/5/6 — the gpt-oss routing investigation (findings)

Recorded here because the conclusion drove a scope decision (see DECISIONS DEC-20).

1. **The brief's proposed fix already exists.** `resolve_server_and_tool` (`mcp.rs:354-373`) does
   unique-suffix recovery via a per-message bare-name map, landed in **b5a4fa7e8 (2026-07-10)**, which
   IS an ancestor of this branch — so it was present in the build the prior worker observed.
2. **Unroutable tool_uses already terminate cleanly.** Synthetic error `tool_result`s are emitted at
   `mcp.rs:2894-2918`, `:2922-2943`, and `:930-952`; none falls through.
3. **The loop is bounded.** `max_iteration` defaults to **10**, not unlimited (`defaults/models.rs:62`),
   and the cap path writes synthetic results and returns `Complete`.
4. **H1 (ambiguous name) is FALSE for `query_rag`.** Probed all three user MCP servers on the live
   `:8080` deployment: `query_rag` is advertised by BioGnosia (`:8081`) ALONE. It is therefore uniquely
   recoverable and the map lookup should hit.
   - *Genuine latent finding:* `validate_input_file` IS advertised by BOTH RCPA (`:9004`) and DSCC
     (`:9006`), so THAT name is correctly marked ambiguous and would legitimately fail to auto-resolve.
     Different tool, different (and arguably correct) behavior.
5. **H2 (map never built) cannot apply.** The two early `Continue` returns (`mcp.rs:1820`, `:1878`) fire
   only when MCP is off or no server is accessible — in which case no tools are advertised, so the model
   has nothing to call.
6. **No live occurrences.** `docker logs` over the last 7 days on the `:8080` stack contains **zero**
   "no valid server_id prefix" warnings.
7. **The evidence stack is gone.** The prior worker's conversations (`1e282c2f-…`) lived on the
   `ziee-review-title` stack at `:18133`, which no longer runs; its database is unavailable.
