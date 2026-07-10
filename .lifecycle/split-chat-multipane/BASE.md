# BASE.md — conflict-surface scoping (split-chat-multipane)

Branch base: `origin/main` @ `28487665d` (verified — worktree HEAD == origin/main).
Written per the feature-lifecycle P3 rule; re-checked by `merge-gate.mjs` at merge.

## Migrations

- **Highest existing migration:** `00000000000135_create_js_tool_settings.sql` (135).
- **This branch adds:** **NONE.** The one operational tunable
  (`PER_USER_MAX_CONNECTIONS`, ITEM-20) is resolved as a **deployment config
  value**, not a settings table (DEC-34) → no migration, so **no numbering
  collision** is possible. If DEC-34 were ever flipped to an admin settings row,
  it would take migration **136** and require the settings CRUD + regen.

## OpenAPI regen

- **Not implied.** The only backend touch (make the chat-stream connection cap
  configurable, `stream/registry.rs`) is an internal const → config value; it adds
  no request/response type, no route, no schema field. So no `just openapi-regen`,
  neither `ui/` nor `desktop/ui/` api-client changes, and `types_ts_parity` is
  unaffected. (Confirms the phase-8 backend chain runs, but the frontend gates are
  NOT triggered *by* the backend change.)

## Contended files — computed correctly (branch's OWN changes vs its merge-base)

**Method (corrected):** the conflict surface is *files a branch's OWN commits
modify that this branch also rewrites* — computed as
`git diff --name-only $(git merge-base origin/main <branch>)..<branch>` intersected
with our rewrite targets. A raw `origin/main..<branch>` (symmetric tip diff)
MISATTRIBUTES main's post-fork additions as the branch's deletions and hugely
inflates stale branches — do NOT use it. (Corrective note: an earlier version of
this file mis-flagged `office-bridge` as removing ~1,185 lines from
`MessageList`/`Chat.store`/`ConversationPage`; that was 100% a tip-diff artifact —
office-bridge is 153 commits behind and its own commits touch those files ZERO
times. Dropped.)

This feature rewrites the most-contended files (`Chat.store.ts`, `MessageList.tsx`,
`ConversationPage.tsx`, `core/extensions/registry.tsx`, `ChatStreamClient.ts`, +
~40 `Stores.Chat` consumers) + one backend file (`stream/registry.rs`). Open
`origin/*` branches whose OWN commits actually edit any of those:

| Branch (behind main) | OWN edits to my rewrite targets | Risk |
|---|---|---|
| feat/chat-empty-completion-notice (29) — same commit carried in fix/anthropic-discover-version (29) + khoi (29) | `Chat.store.ts` **+29**, `MessageList.tsx` **+11** — additive "empty completion notice" feature | **MED-LOW** |
| fix/mobile-and-chat-ui-review (47) | `MessageList.tsx` **+40**, `ConversationPage.tsx` **+6** | **MED-LOW** (ITEM-7 area) |
| feat/scheduled-background-tasks (1) | `chat/stream/registry.rs` **+16** — additive; does NOT touch `PER_USER_MAX_CONNECTIONS` | **LOW** (adjacency only) |
| feat/office-bridge (153) | **none** (own commits touch 0 of my targets) | dropped — stale false alarm |
| feat/containerize-web | **none** own chat edits | dropped — stale false alarm |

Recently-MERGED main churn already absorbed (these ARE our base, not conflicts):
`lazy-load-conversation-messages` + `message-scroll-perf` + `message-scroll-stability`
(the virtualization/window/`MessageViewState` subsystem — the tree-fix) and
`js-tool-scripting` (mcp `run_js`).

## Coordination decision (corrected)

- **No HIGH / competing-rewrite conflict exists.** The real overlaps are all SMALL
  and ADDITIVE — a completion-notice feature (~+40 lines, in 3 branches from one
  commit) and a mobile-UI fix (~+46 lines). My per-pane conversion is a whole-file
  rewrite of `Chat.store.ts`/`MessageList.tsx`/`ConversationPage.tsx`, so when one
  of these merges the small additive delta must be re-applied by hand onto the new
  shape — but there is no structural fight, so the effort is minutes, not a rebase
  of a competing refactor.
- **Backend:** `stream/registry.rs` — `feat/scheduled-background-tasks` adds 16
  lines elsewhere in the file (not the cap), so my `PER_USER_MAX_CONNECTIONS`
  change is adjacency-only and will very likely 3-way auto-merge. LOW.
- **Action for phase 5+:** `git fetch` and re-run the merge-base computation above
  before implementing; re-apply the small completion-notice / mobile deltas if
  they've merged. The `merge-gate.mjs` C4 (stale-branch) + C2 (migration) + C1
  (clean build) re-verify against real main at merge time.
