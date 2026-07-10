# BASE — split-chat-multipane (v2 redesign) conflict surface vs current main

Recorded at re-plan time (P3) so a collision with *current* main is visible now,
not at merge-gate. Supersedes the v1 BASE (base was `28487665d` / migration 135).

## Branch state
- `feat/split-chat-multipane` is **merged current with origin/main** (scheduler +
  notification + artifacts-deliverables folded in). Re-sync immediately before
  implementation — main advances continuously (already 6 ahead again at re-plan).
- This is the **v1→v2 redesign** of the same feature: the per-pane *engine* (v1,
  8/8) is reused unchanged; v2 rebuilds the open/navigate/persist layer on top.

## Migrations
- **FRONTEND-ONLY redesign — adds NO migration.** Highest migration on current
  main is **145** (`create_conversation_deliverables`). No collision possible.

## OpenAPI / types
- **No backend API change → no `openapi.json`/`api-client/types.ts` regen owed.**
  The workspace layout is client-side (localStorage per-user), not a server
  resource. (If persistence is later flipped to server-backed — DEC — it becomes a
  new settings row + regen; called out in DECISIONS.)

## Files main is actively changing that this branch also touches
- `src-app/ui/src/modules/chat/**` — high-traffic (artifacts added canvas /
  right-panel renderers; scheduler added continue-in-chat). v2 edits
  `SplitView.store`, `SplitChatView`, `ConversationPage`, `RecentConversationsWidget`,
  `ConversationCard`, + new picker/tab-strip components. Expect SMALL additive
  overlap on the sidebar widget + right panel; the per-pane engine files are ours.
- Generated `testIds.generated.ts` / gallery `coverage.ts` / `stateMatrix.*` /
  `STATE_MATRIX.md` — both-add conflict on merge, resolved by **regenerating** (as
  in the two merges already done this session). Mechanical, not semantic.
- `src-app/desktop/ui` mirror of the above (ITEM-18, R2-3 diff-review).

## No backend / desktop-tauri surface
- No `src-app/server/**` or `src-app/desktop/tauri/**` → no backend test chain, no
  migration, no `RequirePermissions`, no new permission (so **A9/A10 do not fire** —
  the redesign introduces no permission). Frontend gates only (phase 8).

## Action for phase 5+
- `git fetch` + re-run the merge; re-apply any small chat-module deltas main lands
  meanwhile. `merge-gate.mjs` (C4 stale / C2 migration / C1 clean build / C3
  regen-parity both workspaces) re-verifies against real main at merge time.
