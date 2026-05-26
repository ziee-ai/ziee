# Option A redesign — ProjectDetailPage

**Status**: implemented + tested round 1 (3 of 14 new spec tests failed; fixes
in progress). Plan file written retroactively per `[[feedback_plan_file_then_verify]]`.

## Why

Round-3 layout buried Conversations behind antd `Tabs` alongside Knowledge
and MCP Settings, treating them as equal-weight. Conversations are the 80%
use case on this page. User feedback was direct:

- "MCP settings should not be a top-level tab" (it duplicates the chat
  input modal at a different scope — keep the affordance but demote it)
- "I don't see chat inputs" (no way to start a new conversation inline)
- "Conversations are the most important, why are they hiding in tabs?"

## Final layout (top to bottom)

1. **Header** — back arrow, project title (level-4), Edit, Duplicate
   (no more "New chat" button — inline ChatInput replaces it).
2. **ChatInput section** (`data-test-section="chat-input"`) — embeds the
   chat module's `<ChatInput>` component; on send, creates conversation
   in this project via `Stores.Chat.pendingProjectId` latch + listens
   for `conversation.created` event to navigate to `/chat/{id}`.
3. **Conversations** (`data-test-section="conversations"`) — full-width
   `<ProjectConversationsList>`. No tab wrapper.
4. **Knowledge** (`data-test-section="knowledge"`) — new
   `<ProjectFilesManageDrawer>`: shows up to 5 file chips inline + an
   overflow Tag (`+N more`) + a "Manage" button that opens the full
   `<ProjectFilesPanel>` inside a side `<Drawer>`.
5. **Instructions** (`data-test-section="instructions"`) — inline
   preview; Edit button opens the existing `<ProjectFormDrawer>`.
6. **Description** ("About") (`data-test-section="description"`) —
   rendered ONLY when `project.description` is truthy.
7. **Advanced** (`data-test-section="advanced"`) — defaults summary
   (assistant set/not-set, model set/not-set, MCP approval mode, MCP
   counts) + "Configure MCP defaults" button that opens the shared
   `<McpConfigModal>` in project scope via
   `Stores.Chat.McpStore.openConfigModalForProject(project)`.

## What replaces what

| Old (round 3) | New (Option A) |
|---|---|
| `<Tabs items={[Knowledge, Conversations, MCP Settings]}>` | Six stacked `<section>` blocks |
| Header "New chat" primary button | Inline `<ChatInput>` section |
| Standalone `<ProjectMcpSettingsPanel>` (raw JSON or summary panel) | Advanced section summary + Configure MCP button (opens `McpConfigModal` in project scope) |
| About `<Card>` + Instructions `<Card>` | Plain `<section>` blocks (sections aren't visually heavy boxes) |

## Stores wiring (no schema/API changes)

- `Stores.ProjectDetail.project` — loaded by existing `loadProject` action.
- `Stores.Chat.pendingProjectId` — set on mount by `useEffect`; consumed
  by `Stores.Chat.createConversation` on first send.
- `Stores.EventBus.on('conversation.created', …)` — listener navigates
  to `/chat/{newConvId}` after backend creates the conversation.
- `Stores.Chat.McpStore.openConfigModalForProject(project)` — already
  exists (round-4 modal reuse work). Opens shared `<McpConfigModal>`
  in project scope; saves to `/projects/{id}/mcp-settings`.

## New component

- `src/modules/projects/components/ProjectFilesManageDrawer.tsx` —
  compact inline preview (chips + overflow) + Manage button that
  opens the existing `<ProjectFilesPanel>` in a side `<Drawer>`.

## E2E coverage

Each `data-test-section` is selectable. New + updated specs:

| Spec | What it pins |
|---|---|
| `detail-page-layout.spec.ts` (NEW) | Header buttons, vertical section order, ChatInput render, Conversations not-in-tab, Knowledge inline preview + Manage drawer, Instructions preview, Description conditional, Advanced summary + Configure MCP button, MCP modal project-scope title, no `Save as Default` in project scope |
| `attach-file.spec.ts` (UPDATED) | Empty-state in inline preview, file chip appears post-upload, Manage drawer shows full file list |
| `create-conversation-in-project.spec.ts` (REWRITTEN) | Inline ChatInput render + send creates conversation with `project_id` and navigates |
| `duplicate-project.spec.ts` (UNCHANGED) | Detail-page heading assertion still works on the new layout |
| `edit-project.spec.ts` (UNCHANGED) | Edit drawer flow unchanged |
| `delete-project-leaves-orphan-conversations.spec.ts` (UNCHANGED) | List-page card click still works; detail page not touched |
| `list-page-renders.spec.ts` (UNCHANGED) | List page unaffected |

## Verification protocol (per [[feedback_plan_file_then_verify]])

1. `npx tsc --noEmit` — must be clean.
2. `npm run test:e2e -- --workers=1 tests/e2e/11-projects/` — must be
   all green before commit. (Per `[[feedback_e2e_workers_one]]`.)
3. Re-check against this plan: every entry in "Final layout" has a
   matching `[data-test-section="…"]` attribute on the rendered DOM
   so the layout spec can pin section presence + order.

## Round-1 test result (this iteration)

27 pass / 3 fail in the new `detail-page-layout.spec.ts`:

1. `getByRole('button', { name: /^edit$/i })` failed — antd
   `<EditOutlined />` contributes "edit" to the accessible name so
   the strict-anchor regex misses. Fix: use `/edit/i` (matches the
   round-3 pattern we already applied to the Duplicate button).
2. `renders every section` — failed in beforeEach's `goToProjectsPage`
   waiting for h4 "Projects". Suspected cascade from #1's failure (page
   left in a bad state when previous test screenshot-captured). Re-run
   after fixing #1 should clear this.
3. `Configure MCP defaults opens modal` — `.ant-modal-content`
   strict-mode failure (multiple antd modal portals in the DOM).
   Fix: scope to the visible modal carrying the project-scope title.

## Known follow-ups (NOT in this PR)

- Default-assistant / default-model summary currently shows "Set"/"Not
  set" rather than the actual name. To show names without an extra
  fetch we'd need to subscribe to `Stores.Chat.AssistantStore` +
  `Stores.LlmModel` here; defer until users ask.
- Inline preview limit (5 files) is fixed. If users complain it's too
  small or too large, lift to a config or per-screen-width-responsive
  number.
