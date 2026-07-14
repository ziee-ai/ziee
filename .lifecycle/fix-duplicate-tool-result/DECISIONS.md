# DECISIONS — fix-duplicate-tool-result

Every human/product input the implementation needs, resolved up front. Zero unresolved markers.

### DEC-1: When two `tool_result` blocks share a `tool_use_id` at the chokepoint, which survives?
**Resolution:** Keep the FIRST occurrence; drop later duplicates and `tracing::warn!` the id.
**Basis:** convention — this is exactly the rule `group_assistant_blocks` already applies
internally (`streaming.rs:1746`, `results_by_id.entry(id).or_insert(b)`, commented "Keep the first
result seen for an id; a duplicate is dropped"). Two independent dedup rules in one pipeline would
be a trap. Keep-first is also the only SAFE choice for the wire format: the first occurrence is the
one `group_assistant_blocks` placed in the Tool turn immediately after the Assistant turn, so
keeping it preserves Anthropic's "result immediately after the tool_use" rule, whereas keep-last
could strand the surviving result in a trailing message. Content fidelity is handled by ITEM-1
(which makes the first occurrence the REAL result), so the defense never has to choose between a
placeholder and a real result in practice.

### DEC-2: Should the dedup defense prefer a REAL result over a synthetic `is_error` placeholder?
**Resolution:** No. Keep-first, unconditionally. The defense is validity-only; content correctness
is ITEM-1's job.
**Basis:** codebase — a synthetic placeholder is not distinguishable from a genuine tool error:
`synthetic_missing_tool_result` (`streaming.rs:1649`) sets `is_error: Some(true)`, and so do real
failures (the denied-tool result at `mcp.rs:1493`, the max-iteration errors at `:2063`). A
"prefer non-`is_error`" rule would therefore silently drop REAL failed-tool results in favor of
some other block — strictly worse. Adding a `synthetic: bool` marker field to `ContentBlock` to
tell them apart would touch the shared `ai-providers` wire type for a defense that should never
fire. Rejected as scope creep; the `warn!` is how we learn if it ever fires.

### DEC-3: Where does the dedup defense run?
**Resolution:** In `streaming.rs` at `:468`, immediately BEFORE `clear_old_tool_results` and the
lone `chat_stream` call at `:475`.
**Basis:** codebase — `:475` is the only `chat_stream` call site in the chat module (`title.rs:58`
builds its own separate request and has no tool blocks), and `:468` is after
`call_before_llm_call` (`:396`), so it observes EVERY extension mutation. Ordering before
`clear_old_tool_results` is deliberate: the clearing fn's keep-last-K window is a COUNT over
`positions`, so it must count the true deduped set of results, not phantom duplicates.

### DEC-4: ITEM-4 — claim-then-execute, or keep execute-then-delete?
**Resolution:** Claim-then-execute. DELETE the approval row BEFORE running the tool.
~~If the DELETE fails, SKIP execution and record an `is_error` result so the `tool_use` stays
paired.~~ **REVISED after the blind audit falsified that disposition — see DEC-10.** The
claim-failure branch now propagates the error instead.
**Basis:** user — surfaced as an explicit option picker and chosen by khoi ("Fix all 4 adjacent
defects"), with the trade-off stated in the approved plan and re-stated at approval time: a crash
between the claim and the result append leaves the tool un-run (its `tool_use` then gets a
synthesized `is_error` placeholder on the next turn — protocol-VALID, degraded) rather than
silently re-running an expensive DE analysis and appending a second `tool_result` row. Precedent
exists in the SAME loop: the no-`server_id` arm (`mcp.rs:653`) already deletes the approval row
before erroring out, commented "delete the approval row so the loop can't spin here to
max_iteration (the reported bug)".

### DEC-10: What does each outcome of the claim DELETE authorize? (supersedes DEC-4's failure branch)
**Resolution:** All three outcomes are distinct and total — `claim_outcome()` → `ClaimOutcome`:
- `Ok(true)` → **Won**: we deleted the row, we own this execution.
- `Ok(false)` → **AlreadyClaimed**: zero rows, so a concurrent pass claimed it and is producing
  the result. Skip **silently, emitting nothing** — an error result here would BE the second
  answer for this `tool_use`.
- `Err` → **Failed**: propagate (`return Err`), failing the turn loudly.

**Basis:** codebase + audit. DEC-4's original "skip with an `is_error` result" was **wrong on both
halves**, and the blind audit proved it:
1. `delete_tool_approval` returns `Ok(rows_affected() > 0)` (`approval/repository.rs:390`). The
   bool IS the claim verdict. Discarding it and branching only on `Err` silently turns
   AlreadyClaimed into Won — a double-run of an approved, side-effecting tool. The exactly-once
   property DEC-4 was chosen for was not actually implemented.
2. On `Err` the row **survives**, so `after_llm_call` re-fetches it via
   `get_approved_tools_for_branch` and executes the tool for real — producing a SECOND
   `tool_result` alongside the fabricated `is_error` one. DEC-4's disposition created the very
   duplicate this feature exists to remove.
   Propagating is the only honest option: a failed DELETE means we cannot know whether the row is
   gone, so we can neither safely execute (maybe a double-run) nor safely skip (it may re-execute
   later anyway). A failing DELETE is an unhealthy DB — the result-append that follows would most
   likely fail too — so failing the turn loudly is both correct and consistent with fail-closed
   treatment of an approval (a security boundary).

The user's chosen intent (exactly-once, never silently re-run an expensive tool) is PRESERVED and
strengthened; only the mechanism for the failure branch changed, so this is not a reversal of a
human decision (no HUMAN_FEEDBACK item needed).

### DEC-5: Does this feature introduce an operational tunable (settings row vs fixed constant)?
**Resolution:** No tunable is introduced. Nothing to make admin-configurable.
**Basis:** convention — per the Phase-4 configurable-settings rule, I enumerated every knob the
diff could add: none. The dedup is an unconditional correctness invariant (a wire-format rule
Anthropic enforces), not a policy — making "allow duplicate tool_results" configurable would let
an admin footgun the provider contract. The existing tunables in this file
(`CLEAR_TOOL_RESULTS_TOKEN_THRESHOLD`, `KEEP_LAST_TOOL_RESULTS`, `MAX_KEPT_TOOL_RESULT_CHARS`) are
pre-existing and untouched — promoting them to a settings row is a separate feature, explicitly
out of scope here.

### DEC-6: ITEM-6 — which of the two duplicate unique indexes is dropped?
**Resolution:** Drop migration 114's `idx_message_contents_message_seq_unique`; keep migration
124's named constraint `uq_message_contents_message_sequence`.
**Basis:** convention — a named `CONSTRAINT` produces a clearer violation error than a bare unique
index, and 124 is the later/authoritative statement of intent (its header re-argues the case in
full). Postgres will not permit dropping a constraint's backing index via `DROP INDEX`, so if the
names were ever confused the migration errors loudly rather than silently removing the real guard.
`IF EXISTS` keeps it idempotent for a DB that never ran 114.

### DEC-7: Should `append_content` gain a retry on a `sequence_order` collision?
**Resolution:** No. ITEM-5 corrects the stale comment only; no retry is added.
**Basis:** convention + codebase — the comment itself documents that the sole production caller is
the streaming agentic loop, which "awaits each append in one task" (sequential). A retry would be
dead code guarding a call shape that does not exist, and the plan's minimal-mechanism rule says
propose the smallest thing that fixes the actual defect. If a genuinely concurrent caller ever
appears, the UNIQUE constraint now makes it fail LOUDLY (a hard DB error) instead of silently
colliding — which is the correct failure mode to build a retry against later.

### DEC-8: Should a `UNIQUE (message_id, tool_use_id)` constraint enforce one result per tool_use at the DB layer?
**Resolution:** No.
**Basis:** codebase — `tool_use_id` lives INSIDE the `content` JSONB column
(`message_contents.content`), so this would need a partial expression index over
`(content->>'tool_use_id') WHERE content_type = 'tool_result'` plus a backfill/dedup of existing
production rows. More importantly it would fix the wrong layer: duplicate ROWS are already
harmless (deduped keep-first at `streaming.rs:1746`) — the bug is in request ASSEMBLY, not
storage. Out of scope; recorded here so the option is visibly considered and rejected rather than
overlooked.
