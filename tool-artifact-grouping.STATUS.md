# tool-artifact-grouping — worker status: DONE (follow-up #3 PR open)

## FOLLOW-UP #3 (approval-scroll ACTUALLY works) — PR #135 open
PR: https://github.com/ziee-ai/ziee/pull/135  (base: khoi, head: feat/tool-artifact-grouping). Not merged (human to review/merge).
#134's approval-scroll was a no-op (native `scrollIntoView` on a virtualized OverlayScrollbars
row) and its e2e was a false-green (spied the CALL). Fixed on branch `feat/tool-artifact-grouping`
(reset to `origin/khoi` @ab6127e1), fresh feature-lifecycle **9/9**:
- `ConversationPage` now scrolls to a new `pending_approval` via the app's virtualization-aware
  `messageListRef.scrollToBottom()` (mobile falls back to the `messagesEndRef` end-anchor),
  BYPASSING the `isAtBottom` auto-follow gate that suppressed it. Deduped once per approval;
  mirrors the auto-follow's `!pendingAnchorRef`/`!hasMoreAfter` guards; mount-seed +
  record-seen-before-guard so a leftover approval in the global tool-call map can't yank an
  unrelated conversation. Removed the dead `scrollIntoView`/`scrolledApprovals` Set.
- E2E rewritten to reproduce the real below-the-fold scenario and assert `toBeInViewport` —
  **verified to FAIL without the fix** (external negative check: disabled the scroll → red).
  Blind audit (12 angles) → converged (caught the mobile no-op + missing guards +
  cross-conversation exposure, all fixed). unit 298, 07-mcp e2e 6/6, `npm run check` + `gate:ui`
  clean (same 4 pre-existing base surfaces). A rotating `loginAsAdmin` beforeEach flake on the
  #134 resource-links regression specs is environmental (each test passes individually).

---

## FOLLOW-UP #2 (single-tool wrap + scroll-to-approval) — MERGED (PR #134)

## FOLLOW-UP #2 (single-tool wrap + scroll-to-approval) — PR #134 open
PR: https://github.com/ziee-ai/ziee/pull/134  (base: khoi, head: feat/tool-artifact-grouping). Not merged (human to review/merge).
Branch `feat/tool-artifact-grouping` reset to `origin/khoi` @83e94a6a (my #133 merged),
two follow-up fixes on top, fresh feature-lifecycle **9/9**:
1. A single tool call that produced an artifact (1 or many) is now wrapped in the
   collapsible `McpToolGroupCard` (shared `shouldWrapRun` predicate used by BOTH the
   render branch and `contentSpan` so the run-loop can't desync). The single-tool
   wrapper header shows the tool name + server label; its body renders the file(s)
   (an errored tool keeps its card so the error text stays visible). A single tool
   with no artifact stays the plain card.
2. A pending approval smooth-scrolls into view on mount (once per approval, respects
   `prefers-reduced-motion`), covering both the lone and grouped approval paths.
- Tests: unit 298 pass (`shouldWrapRun` + determinism); e2e 19/19 (new single-artifact
  spec incl. a scrollIntoView spy, + reconciled `mcp-resource-links-{positioning,streaming}`
  for the new wrapped/auto-open layout, + the #133 auto-open specs). `npm run check` +
  `gate:ui` green (same 4 pre-existing base surfaces, my surfaces clean — zero new
  failures). Blind audit (12 angles) → converged; fixed the single-tool header
  duplication + a tautological test.
- Design note in HUMAN_FEEDBACK: the single-tool wrapper shows the files but not the
  tool's input args (deliberate, to avoid duplicating the header) — reviewer may revisit.

---

## PR #133 (original: multi-tool grouping + auto-open) — MERGED into khoi
PR: https://github.com/ziee-ai/ziee/pull/133  (base: khoi, head: feat/tool-artifact-grouping)

Branch: `feat/tool-artifact-grouping` (base `khoi`, off `origin/khoi` @72cbfeee which
already includes the merged resource-link-ssrf PR #131). Worktree:
`/data/khoi/home-workspace/ziee/tmp/tool-artifact-grouping-wt`. **Frontend-only** (no
backend/migration/OpenAPI change).

## What shipped (the 3 issues)
1. **MCP-server artifacts now wrapped in the "N tools called" box.** New pure
   `normalizeToolResultOrder(blocks)` (`chat/core/utils/`) relocates each `tool_result`
   adjacent to its producing `tool_use` by `tool_use_id`, applied in `ChatMessage.tsx`
   after the `sequence_order` sort. The tool run becomes contiguous regardless of where
   the artifact `tool_result` physically landed → `collectToolRun`/`contentSpan`/
   `McpToolGroupCard` (unchanged) fold it in. Works for streaming AND reload. Plus the
   `artifactCreated` fallback is hardened (`resolveArtifactToolUseId`): prefer the
   event's `tool_use_id`; the legacy no-id fallback attributes only when unambiguous and
   only among THIS message's tool_use ids (never guesses "last", never crosses parallel
   tools or conversations).
2. **Group auto-opens while a tool is pending-approval / running.** `McpToolGroupCard`
   now reads live statuses reactively from `Stores.McpComposer.toolCalls`;
   `isExpanded = hasPendingApproval || userOpen`, `userOpen` latches on running/artifact.
   A pending approval FORCES open so a collapsed group can never hide the approval.
3. **Group auto-opens for an artifact** (`hasArtifactInRun`) — the file is visible
   without a click; still user-collapsible when no trigger applies.

## Positional cause (confirmed) — the ordering writeup
The bug is purely positional. During streaming, the non-artifact `tool_result` for a
tool lives in the `McpComposer` store (not in message `contents`); the ONLY `tool_result`
blocks injected into `contents` mid-stream are the synthetic artifact ones from the
`artifactCreated` SSE handler, which **appended at `contents.length`** (end of message).
- **Sandbox artifact** — the sandbox tool_result already carries its `resource_link`, so
  `artifactCreated` MERGEs into the existing adjacent `tool_result` → stays inside the run
  → wrapped. (Persisted order: `[…, tool_use, tool_result(links), …]`, adjacent.)
- **MCP-server artifact** — no inline tool_result exists yet, so `artifactCreated` CREATEs
  a new one appended at the end. If any non-tool block (assistant `text`) sits between the
  tool run and that trailing block, `collectToolRun` (stops at the first non-tool block)
  excludes it → it renders standalone next to the group, **unwrapped**. Persisted repro
  order: `[tool_use_A, tool_result_A, tool_use_B, text, tool_result_B(links)]` → pre-fix
  the group swallows only `[A, result_A, B]` and `result_B` escapes. `normalizeToolResultOrder`
  pulls `result_B` back to right after `tool_use_B` → `[A, result_A, B, result_B, text]` →
  wrapped. This exact order is exercised by the e2e `tool-group-artifact-grouping.spec.ts`.

## Lifecycle + tests
- feature-lifecycle **9/9** (`lifecycle-check --all --base origin/khoi` green); blind
  multi-angle audit (15 angles) → converged, 0 new confirmed findings after the fix round.
  Confirmed fix from the audit: scope the legacy in-flight attribution to the message.
- Unit (`node:test`): 292 pass / 0 fail (22 new across the two helpers).
- E2E (`07-mcp/`, `--workers=1`, isolated ports 19200/19300): **3 pass** —
  artifact-wrapped+auto-open, pending-approval force-open, collapsed-when-no-trigger.
- `npm run check (ui)`: green. `gate:ui`: my new `deep-chat-tool-group` surface + all
  chat deep-states clean; the aggregate gate:ui fails ONLY on 4 PRE-EXISTING base surfaces
  (llm-models-loading / provider-api-key-modal / s3-group-widget / right-panel-file),
  verified identical on `origin/khoi` — zero new failures from this diff.

## Deferred to reviewer
- **Live RCPA/DSCC stack repro** (the DE→pathway `resource_link` flow on the user's live
  containers + local gpt-oss model) — needs the live stack (host :8080 is the user's live
  ziee; not bound). The positional mechanism is proven deterministically via the e2e
  non-adjacent-artifact repro + the ordering analysis above.

Not merged (human to review/merge). `.lifecycle/` stripped in the PR tip; commits authored
khoi <khoi@tinnguyen-lab.com>, no AI attribution.
