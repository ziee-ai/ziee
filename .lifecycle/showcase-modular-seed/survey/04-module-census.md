# 04 — UI Module Census (server `src-app/ui`)

Definitive "which modules need seed" list for the dev gallery. 39 module dirs under
`src-app/ui/src/modules/`. For each: routes (auto-become gallery **pages**), overlays
(Drawer/Dialog/Modal opened via a store action — gallery **overlays** candidates),
user-facing slot contributions, the api-client endpoints its stores fire on load, and
its current gallery seed status.

## How the gallery seeds surfaces (context for the status column)

- **pages** — auto-enumerated at render time from the router store (`pages.tsx` →
  `useResolvedPages`). *Every route a module registers is already a gallery page* — but
  it only renders *populated* if the mock-API **cassette** answers its on-load GET.
  Routes `/`, `/dev/gallery`, `/auth/callback` are skipped (`SKIP_PATHS`). Detail routes
  needing an unresolved required param are skipped (only `providerId`, plus URL-supplied
  `conversationId`/`projectId`, are in `PARAM_VALUES`).
- **cassette** — `fixtures/index.ts` = `crawl.generated.ts` (60 recorded **param-less GET**
  endpoints) overlaid by hand-authored fixtures: **auth, chat, citations, llm-providers,
  project-deep, workflow, skills**. Cassette keys are `Namespace.method` (= `ApiClient.X.y`).
- **overlays** — hand-listed static array in `overlays.tsx` (`OVERLAY_ENTRIES`). An overlay
  NOT in that list never renders in the gallery.
- **deep** — hand-listed `deepStates.tsx`; **chat-only** (ConversationPage transient states).
- **seeded** — hand-listed real-component+store-seed surfaces: `seededSurfaces.tsx`
  (integrator) + `seeded/shard1..5.tsx`.

**Crawl-covered param-less GETs** (the 60): Assistant.{getDefault,list},
AssistantTemplate.{getDefault,list}, Auth.listProviders, AuthProviders.list,
Chat.getUserLlmProviders, Citations.{list,listStyles}, CodeSandbox.{getResourceLimits,listFlavors},
Conversation.list, File.list, FileRagAdmin.get, Hardware.info,
Hub.{getAssistants,getAssistantsVersion,getCatalog,getCatalogVersion,getInstalled,getLocalProviders,getMCPServers,getMCPServersVersion,getModels,getModelsVersion},
LitSearch.{getConnectors,getSettings,listUserKeys}, LlmProvider.{getUserLlmProviders,list,listUserApiKeys},
LlmRepository.list, LocalRuntime.{detectGpu,getRuntimeSettings}, Mcp.getDefaults,
McpServer.listAccessible, McpServerSystem.list, McpToolCall.list, McpUserPolicy.get,
Memory.list, MemoryAdmin.{ftsRebuildStatus,get,rebuildStatus}, MemoryAudit.list,
MemorySettings.get, Onboarding.getProgress, Project.list, RuntimeVersion.{list,usage},
ServerUpdate.getStatus, Skill.list, SkillSystem.list, SummarizationAdmin.get, User.list,
UserGroup.list, WebSearch.{getProviders,getSettings,listUserKeys}, Workflow.{list,listSystem}.

Status legend:
- **SEEDED** — on-load GET(s) covered by crawl/fixture AND its key overlays/surfaces are wired → renders populated.
- **PARTIAL** — main page renders (a load GET is covered) but a notable surface is dark: a detail route needing an unseeded param GET, a secondary page whose GET isn't in the crawl, an unwired overlay, or an uncovered panel/deep-state.
- **UNSEEDED** — no on-load GET covered → the page renders empty/error/crash; nothing wired.
- **INFRA-ONLY** — no reviewable user surface (bootstrap/router/layout/harness).

---

## MODULE-BY-MODULE TABLE

| Module | User surfaces? | Routes | Overlays (wired/total) | On-load endpoints (✓=in crawl/fixture, ✗=missing) | Seed status |
|---|---|---|---|---|---|
| app | yes (SetupPage) | 1 (`/setup`) | 0 | App.getSetupStatus ✗ | **PARTIAL** (form renders; getSetupStatus uncovered) — mostly bootstrap infra (`routerEffects`) |
| assistant | yes | 2 (`/settings/assistants`, `/settings/assistant-templates`) | 1/1 (AssistantFormDrawer ✓) | Assistant.list ✓, AssistantTemplate.list ✓ | **SEEDED** |
| auth | yes | 4 (`/auth`, `/auth/callback`†, `/auth/link-account`, `/settings/sessions`) | 0 | Auth.listProviders ✓, Auth.me ✓(fixture), Auth.getSessionSettings ✗ | **SEEDED** (fixture + many seeded surfaces) — gap: `/settings/sessions` (SessionSettingsPage) GET uncovered |
| auth-providers | yes | 1 (`/settings/auth-providers`) | 1/1 (AuthProviderEditDrawer ✓) | AuthProviders.list ✓ | **SEEDED** |
| chat | yes (flagship) | 4 (`/`†, `/chat`, `/chat/:conversationId`, `/chats`) | 0 (uses deep-states) | Conversation.list ✓, Conversation.get/Message.getHistory/Branch.list ✓(fixture) | **SEEDED** (17 deep-states + fixtures + widgets) |
| citations | yes | 1 (`/settings/citations`) | 1/1 (ImportCitationsModal ✓) | Citations.list ✓, Citations.listStyles ✓ | **SEEDED** |
| code-sandbox | yes | 1 (`/settings/sandbox`) | 0 | CodeSandbox.getResourceLimits ✓, listFlavors ✓, **listRootfsVersions ✗** | **SEEDED** (3 seeded section surfaces) — full page's rootfs section GET uncovered |
| config-client | no | 0 | 0 | — (client-side config only) | **INFRA-ONLY** |
| dev-gallery | no | 1 (`/dev/gallery`†) | 0 | — | **INFRA-ONLY** (the harness itself) |
| file | yes | 1 (`/files/:fileId`) | 1/1 (FilePreviewDrawer ✓) | File.list ✓, **File.get(param) ✗** for the viewer route | **SEEDED** (heavy shard2: cards/csv/xlsx/pdf/viewer states) — full `/files/:fileId` route needs File.get seed |
| file-rag | yes | 1 (`/settings/file-rag-admin`) | 0 | FileRagAdmin.get ✓, LlmModel.list ✓ | **SEEDED** (+ seeded save-error surface) |
| hardware | yes | 2 (`/hardware-monitor`, `/settings/hardware`) | 0 | Hardware.info ✓, Hardware.stream (SSE→seeded) | **SEEDED** (monitor shadow + error + no-gpu seeds) |
| hub | yes | 1 (`/hub/:activeTab?`) | 5/5 (assistant/model/mcp/skill/workflow details ✓) | Hub.{getInstalled,getCatalog,getModels,getAssistants,getMCPServers,getLocalProviders} ✓ | **SEEDED** |
| js-tool | yes | 1 (`/settings/js-tool`) | 0 | **JsTool.getSettings ✗** | **UNSEEDED** |
| knowledge-base | yes | 2 (`/knowledge`, `/knowledge/:kbId`) | 0/1 (**KnowledgeBaseFormDrawer NOT wired**) | **KnowledgeBase.list ✗, KnowledgeBase.get(param) ✗** | **UNSEEDED** (also registers `kb_source` right-panel renderer — no deep-state) |
| layouts | no | 0 | 0 (app-layout mobile Drawer = infra) | — | **INFRA-ONLY** (AppLayout/Settings shell chrome) |
| literature | yes | 2 (`/settings/literature`, `/settings/literature-keys`) | 0 | LitSearch.getSettings ✓, getConnectors ✓, listUserKeys ✓ | **SEEDED** (+ seeded connectors-loading, lit-tool-result, literature right-panel deep-state) |
| llm-local-runtime | yes | 1 (`/settings/llm-runtime`) | 1/1 (RuntimeDownloadDrawer ✓) | LocalRuntime.getRuntimeSettings ✓, detectGpu ✓, RuntimeVersion.list ✓, RuntimeVersion.usage ✓ | **SEEDED** (heavy shard3 + live-logs seed) |
| llm-provider | yes | 1 (`/settings/llm-providers/:providerId?`) | 6/6 (provider + 4 model drawers + group-assign ✓) | LlmProvider.list ✓, getUserLlmProviders ✓, LlmModel.list ✓ | **SEEDED** (hand fixture + seeded header/models/download-indicator) |
| llm-repository | yes | 1 (`/settings/llm-repositories`) | 1/1 (LlmRepositoryDrawer ✓) | LlmRepository.list ✓ | **SEEDED** |
| mcp | yes | 2 (`/settings/mcp-admin`, `/settings/mcp-servers`) | 3/3 (McpServerDrawer, McpConfigModal, GroupSystemMcp ✓) | McpServer.listAccessible ✓, McpServerSystem.list ✓, Mcp.getDefaults ✓, McpUserPolicy.get ✓, McpToolCall.list ✓ | **SEEDED** (+ shard4 policy/kv + tool-calls seeds) |
| memory | yes | 2 (`/settings/memory`, `/settings/memory-admin`) | 0 | Memory.list ✓, MemorySettings.get ✓, MemoryAdmin.get ✓, MemoryAudit.list ✓ | **SEEDED** (+ core-memory + audit-loaded seeds) |
| notification | yes | 1 (`/notifications`) | 0 | **Notification.list ✗, Notification.unreadCount ✗** | **UNSEEDED** (page + `sidebarBottom` bell widget both dark) |
| onboarding | yes | 1 (`/onboarding`) | 0 | Onboarding.getProgress ✓ (+ LlmProvider.getUserLlmProviders ✓, McpServerSystem.list ✓, Hub.getMCPServers ✓) | **PARTIAL** (getProgress covered; multi-step guide surfaces uncovered) |
| profile | yes | 1 (`/settings/profile`) | 0 | reads Auth store (Auth.me ✓) | **SEEDED** (renders from seeded Auth store) |
| projects | yes | 3 (`/projects`, `/projects/:projectId`, `/projects/:projectId/chat/:conversationId`) | 2/2 (ProjectFormDrawer, AddToProjectModal ✓) | Project.list ✓, Project.get/listConversations ✓(fixture) | **SEEDED** (project-deep fixture + 3 detail seeds) |
| router | no | 0 | 0 | — | **INFRA-ONLY** (RouterComponent) |
| scheduler | yes | 2 (`/scheduled-tasks`, `/settings/scheduler`) | 0/1 (**ScheduledTaskFormDrawer NOT wired**) | **ScheduledTask.list ✗, SchedulerAdminSettings.get ✗** | **UNSEEDED** (also `sidebarNavigation` entry) |
| server-update | yes | 1 (`/settings/about`) | 0 | ServerUpdate.getStatus ✓ | **SEEDED** (+ `appBanners` slot) |
| settings | shell | 1 (`/settings`) | 0 | — (redirect) | **INFRA-ONLY** — but the `settings` seeded shadow renders the nav shell; hosts every `settings*Pages` slot |
| settings-general | yes | 1 (`/settings/general`) | 0 | client config (ConfigClient store) | **SEEDED** (rendered via the `settings` landing shadow; client-side appearance) |
| skill | yes | 2 (`/settings/skills`, `/settings/skills-admin`) | 4/4 (SkillConversationDrawer, SkillDetailDrawer, ImportSkillDialog, GroupSystemSkills ✓) | Skill.list ✓, SkillSystem.list ✓ | **SEEDED** (skills fixture + conversation-skills seeds) |
| summarization | yes | 1 (`/settings/summarization-admin`) | 0 | SummarizationAdmin.get ✓, LlmModel.list ✓ | **SEEDED** |
| user | yes | 2 (`/settings/users`, `/settings/user-groups`) | 7/7 (create/edit/reset-pw/user-groups/assign-group + group edit/members ✓) | User.list ✓, UserGroup.list ✓ | **SEEDED** |
| user-llm-providers | yes | 1 (`/settings/user-llm-providers`) | 1/1 (ProviderApiKeyModal ✓) | LlmProvider.getUserLlmProviders ✓, listUserApiKeys ✓ | **SEEDED** (also owns the chat ModelPicker) |
| user-profile | yes (footer widget) | 0 | 0 | reads Auth store (Auth.me ✓) | **SEEDED** (shard5 user-profile widget loading + loaded) |
| voice | yes | 1 (`/settings/voice`) | 0/1 (**UploadModelDrawer NOT wired**) | **Voice.{listModels,getSettings,listVersions,listModelCatalog,getInstance} ✗** | **UNSEEDED** |
| web-search | yes | 2 (`/settings/web-search`, `/settings/web-search-keys`) | 0 | WebSearch.getSettings ✓, getProviders ✓, listUserKeys ✓ | **SEEDED** (+ seeded global/providers-loading) |
| workflow | yes | 2 (`/settings/workflows`, `/settings/workflows-admin`) | 6/6 (detail/import/run/dry-run/tests-panel + group-assign ✓) | Workflow.list ✓, Workflow.listSystem ✓ | **SEEDED** (workflow fixture + heavy shard1) |

† route skipped from the gallery page grid (`SKIP_PATHS`: `/`, `/auth/callback`, `/dev/gallery`).

---

## Roll-up

- **UNSEEDED — the top priorities to seed (5):** `js-tool`, `knowledge-base`, `notification`,
  `scheduler`, `voice`. None of their on-load GETs are in the crawl cassette, so their pages
  render empty/error/crash today. `knowledge-base`, `scheduler`, `voice` each also ship an
  **unwired overlay** (KnowledgeBaseFormDrawer / ScheduledTaskFormDrawer / UploadModelDrawer).
- **PARTIAL (2):** `app` (SetupPage — getSetupStatus uncovered), `onboarding` (guide steps
  beyond getProgress). Plus SEEDED-with-a-gap noted inline: `auth` (`/settings/sessions`),
  `code-sandbox` (`listRootfsVersions` for the full page), `file` (`File.get` for `/files/:fileId`).
- **SEEDED (24):** assistant, auth, auth-providers, chat, citations, code-sandbox, file, file-rag,
  hardware, hub, literature, llm-local-runtime, llm-provider, llm-repository, mcp, memory, profile,
  projects, server-update, settings-general, skill, summarization, user, user-llm-providers, workflow,
  user-profile. (Hand-authored fixtures back: auth, chat, citations, llm-providers, project-deep,
  workflow, skills.)
- **INFRA-ONLY (5):** config-client, dev-gallery, layouts, router, settings (settings hosts the
  `settings*Pages` slots and has a seeded nav-shell shadow, but registers no reviewable content route of its own).

### Unwired overlays (exist in a module, absent from `overlays.tsx`)
| Overlay | Module | Store open action |
|---|---|---|
| `KnowledgeBaseFormDrawer` | knowledge-base | `Stores.KnowledgeBaseComposer` (via `components` mount) |
| `ScheduledTaskFormDrawer` | scheduler | `Stores.SchedulerDrawer.open` (via `components` mount) |
| `UploadModelDrawer` | voice | voice model store |

### User-facing slot contributions (render inside a parent page/layout, not their own route)
| Slot | Contributors |
|---|---|
| `sidebarPrimaryActions` | chat (New Chat) |
| `sidebarNavigation` | chat (Chats), knowledge-base (Knowledge), projects (Projects widget), scheduler (Scheduled Tasks) |
| `sidebarContent` | chat (RecentConversationsWidget) |
| `sidebarTools` | hub (Hub), onboarding, settings |
| `sidebarBottom` | notification (NotificationBellWidget) |
| `sidebarFooter` | user-profile (UserProfileWidget) |
| `settingsUserPages` | assistant, citations, literature, mcp, memory, profile, settings-general, skill, user-llm-providers, web-search, workflow |
| `settingsAdminPages` | assistant, auth (Sessions), auth-providers, code-sandbox, file-rag, hardware, js-tool, literature, llm-local-runtime, llm-provider, llm-repository, mcp, memory, scheduler, server-update, skill, summarization, user, voice, web-search, workflow |
| `appBanners` | server-update |
| `routerEffects` | app, onboarding |
| `components` (headless mounts) | notification (toast listener), scheduler (form drawer), router |
| right-panel renderers (`registerPanelRenderer`) | file (`file` — deep-state ✓), literature (`literature` — deep-state ✓), knowledge-base (`kb_source` — **no deep-state**) |

*Every `settings*Pages` entry becomes a menu item in the `SettingsPage` nav shell; its page
content is the module's `/settings/<section>` route (already an enumerated gallery page). The
`settings` seeded landing renders the nav shell itself.*

---

## Desktop vs server UI delta (`src-app/desktop/ui/src/modules/`)

Desktop loads **all** shared server-ui modules via `@ziee/ui-core` + its own glob, then adds
desktop-specific modules. So the server-ui gallery does **not** cover these **desktop-only**
modules (they register their own routes/slots and would each need seed in a *desktop* gallery):

| Desktop-only module | Routes | Notes |
|---|---|---|
| `desktop-base` | — | base desktop wiring (no route) |
| `file-dialog` | — | native file dialog bridge (no route) |
| `host-mount` | `/settings/host-mount` + `host-mount` section; `chatConversationHeaderTrailing` slot | host filesystem mounts |
| `memory` (desktop override) | `/settings/memory-combined` + `settingsUserPages` | **shadows** the shared memory settings page (combined user+admin) |
| `remote-access` | `/settings/remote-access` + `settingsAdminPages` | tunnel/remote access |
| `tunnel-auth` | `/auth/magic`, `/auth/magic/:token` | magic-link login |
| `updater` | `/settings/about` + `settingsUserPages` + `sidebarFooter` | **shadows** server-update's About; adds a footer widget |
| `window` | `/settings/window` | desktop window prefs |
| `layouts` (desktop override) | — | desktop layout chrome (title bar etc.) |

No module exists in the server ui that is *missing* from desktop — desktop is a superset (shared
+ desktop-only). `updater` and desktop-`memory` deliberately shadow server modules (`server-update`,
`memory`).
