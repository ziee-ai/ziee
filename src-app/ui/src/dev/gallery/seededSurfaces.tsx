/**
 * Seeded-surface entries — real module pages/components rendered with a
 * mount-time STORE SEED that reaches branches the GET-driven data-state pass
 * (empty/error/delayed) structurally cannot.
 *
 * The data-state pass drives the wire (a GET 500 / a latency / an emptied body),
 * so it reaches the `!data` early-returns (load spinner, load-error status). But
 * many branches only render once data is ALREADY LOADED and a *mutation* then
 * fails — e.g. a section's inline "save failed" alert (`data && error`), or a
 * post-load empty derived from seeded state. A GET-only harness never issues the
 * failing mutation, so those arms stay dark.
 *
 * A seeded surface renders the SAME real component inside an isolated
 * `MemoryRouter`, lets it load normally (loaded cassette), then a `setup()` seeds
 * the transient piece through the REAL store (`Store.store.setState(...)`) — the
 * exact channel deepStates/overlays already use. Driven one-per-page-load via
 * `?surface=<slug>` so each seeded singleton store never bleeds across entries.
 *
 * A seeded slug MAY intentionally SHADOW an enumerated page slug (a seeded entry
 * is resolved before the enumerated page in `GalleryPages`): use this when the
 * enumerated route is structurally unreviewable in the GET-only harness — a
 * route whose content lives in its LAYOUT redirect (`/settings` → the settings
 * landing), whose live data arrives over SSE not a JSON GET (`/hardware-monitor`
 * usage), or that needs a query param the browse enumerator can't fill
 * (`/auth/link-account?link_token=…`). The seeded entry renders the real page in
 * its reviewable state; the enumerated (blank) form stays only on the browse
 * canvas, where per-surface capture never lands.
 */
import { type ReactNode, Suspense, useEffect } from 'react'
import { MemoryRouter, Route, Routes } from 'react-router-dom'
import { Text, Title } from '@/components/ui'
import { AppErrorBoundary } from '@/components/AppErrorBoundary'
import { Loading } from '@/core/components/Loading'
import { useRunInteraction } from './interactions'
import {
  type SeededSurfaceEntry,
  lazyCompose,
  lazyNamed,
  lazyProps,
  holdPatch,
  holdForever,
  whenTrue,
} from './seeded/helpers'
import {
  firstEnabledRemoteProviderId,
  llmProvidersList,
} from './fixtures/llm-providers'
import {
  DEEP_PROJECT_ID,
  deepProject,
  deepProjectConversations,
  deepProjectFiles,
} from './fixtures/project-deep'
// Per-shard entry lists (parallel grind). Each shard owns ONLY its own file;
// this aggregator is integrator-owned. Add a shard import + spread below.
import { shard1Seeded } from './seeded/shard1'
import { shard2Seeded } from './seeded/shard2'
import { shard3Seeded } from './seeded/shard3'
import { shard4Seeded } from './seeded/shard4'
import { shard5Seeded } from './seeded/shard5'

export type { SeededSurfaceEntry }

/**
 * Seed the ProjectDetail + ProjectFiles stores for the full-page
 * `ProjectDetailPage` surface. `loadProject` fires on mount (setting loading +
 * loading conversations from the thin cassette); `holdPatch` re-asserts the rich
 * fixture over it so the loaded page renders its populated form.
 */
async function seedProjectDetail(patch: {
  project: typeof deepProject | null
  conversations?: (typeof deepProjectConversations)[number][]
  files?: (typeof deepProjectFiles)[number][]
  error?: string | null
}): Promise<void> {
  const { ProjectDetail } = await import(
    '@/modules/projects/stores/ProjectDetail.store'
  )
  const { ProjectFiles } = await import(
    '@/modules/file/project-extension/stores/ProjectFiles.store'
  )
  await holdPatch(() => {
    ProjectDetail.store.setState({
      project: patch.project,
      loading: false,
      error: patch.error ?? null,
      conversations: patch.conversations ?? [],
      conversationsLoading: false,
      conversationsLoadingMore: false,
      conversationsHasMore: false,
      conversationsError: null,
    } as any)
    ProjectFiles.store.setState({
      currentProjectId: patch.project?.id ?? null,
      files: patch.files ?? [],
      filesLoading: false,
      error: null,
    } as any)
  })
}

const RENAME_PROVIDER = llmProvidersList.providers[0]

/** Integrator-owned entries (batches 1-3). Shard entries are concatenated below. */
const integratorSeeded: SeededSurfaceEntry[] = [
  // ── ProviderHeader inline RENAME form — an INTERACTION-gated surface. The header
  //    renders the name as a Title with an edit (pencil) button; clicking it swaps
  //    in the inline `layout="inline"` rename Form. The `rename` recipe drives that
  //    click so the capture pass shoots the inline form (the A10 collapsed-input /
  //    vertical-form bug family — same as chat TitleEditor). ──────────────────────
  {
    slug: 'seeded-interact-provider-header',
    title: 'Provider header — inline rename (interaction)',
    note: 'click the edit pencil → the inline provider-name rename form (A10 / vertical-form bug family)',
    path: '/settings/llm-providers/:providerId',
    initialPath: `/settings/llm-providers/${RENAME_PROVIDER?.id ?? 'p1'}`,
    component: lazyNamed(
      () => import('@/modules/llm-provider/components/ProviderHeader'),
      'ProviderHeader',
    ),
    setup: async () => {
      const { LlmProviderStoreDef } = await import(
        '@/modules/llm-provider/stores/LlmProvider.store'
      )
      // Keep the provider seeded so ProviderHeader's find(id) resolves through the
      // recipe's click (holdForever: the lazy chunk may mount after a fixed hold).
      holdForever(() =>
        LlmProviderStoreDef.store.setState({
          providers: llmProvidersList.providers,
          loading: false,
          isInitialized: true,
        } as any),
      )
    },
    interactions: [
      {
        name: 'rename',
        note: 'click edit → inline rename form appears (renders the collapsed-input / vertical-form bug visible)',
        steps: async d => {
          await d.click('llm-provider-header-edit-name-btn')
          await d.waitFor('llm-provider-header-name-input', 3000)
          await d.wait(300)
        },
      },
    ],
  },
  // ── FULL-PAGE ProjectDetailPage — the priority life-science surface. The
  // enumerated `/projects/:projectId` page renders from a thin cassette (no
  // conversations, no files); these seed the REAL ProjectDetail + ProjectFiles
  // stores so the populated page (instructions, conversation list, knowledge
  // files) is reviewable. loaded(rich) / empty / error. ────────────────────────
  {
    slug: 'deep-project-detail',
    title: 'Project detail — loaded (rich)',
    note: 'a fully-populated project: instructions + description + a conversation list + attached knowledge files',
    path: '/projects/:projectId',
    initialPath: `/projects/${DEEP_PROJECT_ID}`,
    component: lazyNamed(
      () => import('@/modules/projects/pages/ProjectDetailPage'),
      'ProjectDetailPage',
    ),
    setup: () =>
      seedProjectDetail({
        project: deepProject,
        conversations: deepProjectConversations,
        files: deepProjectFiles,
      }),
  },
  {
    slug: 'deep-project-detail-empty',
    title: 'Project detail — empty (no chats, no files)',
    note: 'a loaded project with zero conversations + zero knowledge files → the empty affordances',
    path: '/projects/:projectId',
    initialPath: `/projects/${DEEP_PROJECT_ID}`,
    component: lazyNamed(
      () => import('@/modules/projects/pages/ProjectDetailPage'),
      'ProjectDetailPage',
    ),
    setup: () =>
      seedProjectDetail({
        project: { ...deepProject, description: undefined, instructions: undefined },
        conversations: [],
        files: [],
      }),
  },
  {
    slug: 'deep-project-detail-error',
    title: 'Project detail — load error',
    note: 'load settled with no project → the recoverable "Failed to load project" Result',
    path: '/projects/:projectId',
    initialPath: `/projects/${DEEP_PROJECT_ID}`,
    component: lazyNamed(
      () => import('@/modules/projects/pages/ProjectDetailPage'),
      'ProjectDetailPage',
    ),
    setup: () =>
      seedProjectDetail({
        project: null,
        error: 'The project could not be loaded.',
      }),
  },
  // ── file_rag admin: 5 section cards share Stores.FileRagAdmin. Once settings
  // load, seeding `.error` flips every section's inline save-error alert. ──────
  {
    slug: 'seeded-file-rag-error',
    title: 'Document RAG admin — save error (all sections)',
    note: 'settings loaded, then Stores.FileRagAdmin.error set → every section inline error alert',
    path: '/settings/file-rag-admin',
    initialPath: '/settings/file-rag-admin',
    component: lazyNamed(
      () => import('@/modules/file-rag/pages/FileRagAdminPage'),
      'FileRagAdminPage',
    ),
    setup: async () => {
      const { FileRagAdmin } = await import(
        '@/modules/file-rag/stores/FileRagAdmin.store'
      )
      await whenTrue(() => FileRagAdmin.store.getState().settings != null)
      await holdPatch(() =>
        FileRagAdmin.store.setState({
          error: 'Failed to save Document RAG settings.',
        } as any),
      )
    },
  },
  // ── code_sandbox resource limits section (behind a non-default tab, so the
  // page pass never mounts it): rendered direct, limits loaded, then error. ────
  {
    slug: 'seeded-sandbox-limits-error',
    title: 'Code Sandbox limits — save error',
    note: 'limits loaded, then Stores.SandboxResourceLimits.error → inline error alert',
    path: '/',
    initialPath: '/',
    component: lazyNamed(
      () => import('@/modules/code-sandbox/components/SandboxResourceLimitsSection'),
      'SandboxResourceLimitsSection',
    ),
    setup: async () => {
      const { SandboxResourceLimits } = await import(
        '@/modules/code-sandbox/stores/SandboxResourceLimits.store'
      )
      await whenTrue(() => SandboxResourceLimits.store.getState().limits != null)
      await holdPatch(() =>
        SandboxResourceLimits.store.setState({
          error: 'Failed to save resource limits.',
        } as any),
      )
    },
  },
  // ── code_sandbox resource limits: stuck loading (loading && !limits). ────────
  {
    slug: 'seeded-sandbox-limits-loading',
    title: 'Code Sandbox limits — loading',
    note: 'loading && !limits → the resource-limits load spinner',
    path: '/',
    initialPath: '/',
    component: lazyNamed(
      () => import('@/modules/code-sandbox/components/SandboxResourceLimitsSection'),
      'SandboxResourceLimitsSection',
    ),
    setup: async () => {
      const { SandboxResourceLimits } = await import(
        '@/modules/code-sandbox/stores/SandboxResourceLimits.store'
      )
      await holdPatch(() =>
        SandboxResourceLimits.store.setState({ loading: true, limits: null } as any),
      )
    },
  },
  // ── web_search sections (rendered direct): stuck loading (both arms). ────────
  {
    slug: 'seeded-web-search-loading',
    title: 'Web Search settings — loading',
    note: 'loading && !settings / loading && providers.length===0 → both section loaders',
    path: '/',
    initialPath: '/',
    component: lazyCompose([
      {
        loader: () => import('@/modules/web-search/components/WebSearchGlobalSection'),
        name: 'WebSearchGlobalSection',
      },
      {
        loader: () => import('@/modules/web-search/components/WebSearchProvidersSection'),
        name: 'WebSearchProvidersSection',
      },
    ]),
    setup: async () => {
      const { WebSearchAdmin } = await import(
        '@/modules/web-search/stores/WebSearchAdmin.store'
      )
      await holdPatch(() =>
        WebSearchAdmin.store.setState({
          loading: true,
          settings: null,
          providers: [],
        } as any),
      )
    },
  },
  // ── lit_search connectors section: stuck loading (loading && no connectors). ─
  {
    slug: 'seeded-literature-loading',
    title: 'Literature settings — loading',
    note: 'loading && connectors.length===0 → the connectors-section loader',
    path: '/',
    initialPath: '/',
    component: lazyNamed(
      () => import('@/modules/literature/components/settings/LitSearchConnectorsSection'),
      'LitSearchConnectorsSection',
    ),
    setup: async () => {
      const { LitSearchAdmin } = await import(
        '@/modules/literature/stores/LitSearchAdmin.store'
      )
      await holdPatch(() =>
        LitSearchAdmin.store.setState({
          loading: true,
          settings: null,
          connectors: [],
        } as any),
      )
    },
  },
  // ── DownloadIndicatorWidget: no active/failed downloads → the empty return
  // (a header widget, never on an enumerated page). Default store is empty. ────
  {
    slug: 'seeded-download-indicator-empty',
    title: 'Download indicator — empty',
    note: 'activeDownloads.length===0 && failedDownloads.length===0 → renders nothing',
    path: '/',
    initialPath: '/',
    component: lazyNamed(
      () => import('@/modules/llm-provider/components/widgets/DownloadIndicatorWidget'),
      'DownloadIndicatorWidget',
    ),
    setup: async () => {
      const { LlmModelDownload } = await import(
        '@/modules/llm-provider/stores/LlmModelDownload.store'
      )
      await holdPatch(() =>
        LlmModelDownload.store.setState({ downloads: [] } as any),
      )
    },
  },
  // ── RecentConversationsWidget: loading (loading && !isInitialized). ──────────
  {
    slug: 'seeded-recent-convos-loading',
    title: 'Recent chats widget — loading',
    note: 'loading && !isInitialized → the loading spinner',
    path: '/',
    initialPath: '/',
    component: lazyNamed(
      () => import('@/modules/chat/widgets/RecentConversationsWidget'),
      'RecentConversationsWidget',
    ),
    setup: async () => {
      const { ChatHistory } = await import(
        '@/modules/chat/stores/ChatHistory.store'
      )
      await holdPatch(() =>
        ChatHistory.store.setState({ loading: true, isInitialized: false } as any),
      )
    },
  },
  // ── RecentConversationsWidget: empty (!loading && no conversations). ─────────
  {
    slug: 'seeded-recent-convos-empty',
    title: 'Recent chats widget — empty',
    note: '!loading && recentConversations.length===0 → the empty state',
    path: '/',
    initialPath: '/',
    component: lazyNamed(
      () => import('@/modules/chat/widgets/RecentConversationsWidget'),
      'RecentConversationsWidget',
    ),
    setup: async () => {
      const { ChatHistory } = await import(
        '@/modules/chat/stores/ChatHistory.store'
      )
      await holdPatch(() =>
        ChatHistory.store.setState({
          loading: false,
          isInitialized: true,
          recentConversations: [],
        } as any),
      )
    },
  },
  // ── LiveLogsPanel: no log output yet → the empty state (prop modelId). ───────
  {
    slug: 'seeded-live-logs-empty',
    title: 'Local runtime live logs — empty',
    note: 'no log lines yet → "No log output yet" empty',
    path: '/',
    initialPath: '/',
    component: lazyProps(
      () => import('@/modules/llm-local-runtime/components/LiveLogsPanel'),
      'LiveLogsPanel',
      { modelId: 'gallery-model-1' },
    ),
  },
  // ── WorkflowRunsList: no runs for this workflow → empty (prop workflowId). ───
  {
    slug: 'seeded-workflow-runs-empty',
    title: 'Workflow runs list — empty',
    note: '!loading[wf] && items.length===0 → the empty state',
    path: '/',
    initialPath: '/',
    component: lazyProps(
      () => import('@/modules/workflow/components/WorkflowRunsList'),
      'WorkflowRunsList',
      { workflowId: 'wf-1', onSelectRun: () => undefined },
    ),
    setup: async () => {
      const { WorkflowRuns } = await import(
        '@/modules/workflow/stores/WorkflowRuns.store'
      )
      await holdPatch(() =>
        WorkflowRuns.store.setState({
          runs: { 'wf-1': [] },
          loading: { 'wf-1': false },
        } as any),
      )
    },
  },
  // ── ConversationSkillsPanel: loading / error / empty (prop conversationId). ──
  {
    slug: 'seeded-conversation-skills-loading',
    title: 'Conversation skills — loading',
    note: 'loading && !available → the load spinner',
    path: '/',
    initialPath: '/',
    component: lazyProps(
      () => import('@/modules/skill/components/ConversationSkillsPanel'),
      'ConversationSkillsPanel',
      { conversationId: 'conv-1' },
    ),
    setup: async () => {
      const { ConversationSkills } = await import(
        '@/modules/skill/stores/ConversationSkills.store'
      )
      await holdPatch(() =>
        ConversationSkills.store.setState({
          available: {},
          loading: { 'conv-1': true },
          error: null,
        } as any),
      )
    },
  },
  {
    slug: 'seeded-conversation-skills-error',
    title: 'Conversation skills — error',
    note: 'error && !available → the error state',
    path: '/',
    initialPath: '/',
    component: lazyProps(
      () => import('@/modules/skill/components/ConversationSkillsPanel'),
      'ConversationSkillsPanel',
      { conversationId: 'conv-1' },
    ),
    setup: async () => {
      const { ConversationSkills } = await import(
        '@/modules/skill/stores/ConversationSkills.store'
      )
      await holdPatch(() =>
        ConversationSkills.store.setState({
          available: {},
          loading: { 'conv-1': false },
          error: 'Failed to load skills.',
        } as any),
      )
    },
  },
  {
    slug: 'seeded-conversation-skills-empty',
    title: 'Conversation skills — empty',
    note: 'available loaded but allRows.length===0 → the empty state',
    path: '/',
    initialPath: '/',
    component: lazyProps(
      () => import('@/modules/skill/components/ConversationSkillsPanel'),
      'ConversationSkillsPanel',
      { conversationId: 'conv-1' },
    ),
    setup: async () => {
      const { ConversationSkills } = await import(
        '@/modules/skill/stores/ConversationSkills.store'
      )
      const { SkillStoreDef } = await import('@/modules/skill/stores/Skill.store')
      await holdPatch(() => {
        SkillStoreDef.store.setState({ skills: [] } as any)
        ConversationSkills.store.setState({
          available: { 'conv-1': [] },
          loading: { 'conv-1': false },
          error: null,
        } as any)
      })
    },
  },
  // ── CoreMemoryBlocksEditor: loading / empty (prop assistantId). ──────────────
  {
    slug: 'seeded-core-memory-loading',
    title: 'Core memory blocks — loading',
    note: 'blocks empty && loading → the load spinner',
    path: '/',
    initialPath: '/',
    component: lazyProps(
      () => import('@/modules/memory/components/CoreMemoryBlocksEditor'),
      'CoreMemoryBlocksEditor',
      { assistantId: 'asst-1' },
    ),
    setup: async () => {
      const { CoreMemoryBlocks } = await import(
        '@/modules/memory/stores/CoreMemoryBlocks.store'
      )
      await holdPatch(() =>
        CoreMemoryBlocks.store.setState({
          blocksByAssistant: { 'asst-1': [] },
          loadingByAssistant: { 'asst-1': true },
        } as any),
      )
    },
  },
  {
    slug: 'seeded-core-memory-empty',
    title: 'Core memory blocks — empty',
    note: 'blocks empty && !loading → "No blocks yet" empty',
    path: '/',
    initialPath: '/',
    component: lazyProps(
      () => import('@/modules/memory/components/CoreMemoryBlocksEditor'),
      'CoreMemoryBlocksEditor',
      { assistantId: 'asst-1' },
    ),
    setup: async () => {
      const { CoreMemoryBlocks } = await import(
        '@/modules/memory/stores/CoreMemoryBlocks.store'
      )
      await holdPatch(() =>
        CoreMemoryBlocks.store.setState({
          blocksByAssistant: { 'asst-1': [] },
          loadingByAssistant: { 'asst-1': false },
        } as any),
      )
    },
  },
  // ── McpToolCallsTab: load error (prop serverId). ─────────────────────────────
  {
    slug: 'seeded-mcp-tool-calls-error',
    title: 'MCP tool calls — error',
    note: 'Stores.McpToolCalls.error → the danger text',
    path: '/',
    initialPath: '/',
    component: lazyProps(
      () => import('@/modules/mcp/components/common/McpToolCallsTab'),
      'McpToolCallsTab',
      { serverId: 'srv-1' },
    ),
    setup: async () => {
      const { McpToolCalls } = await import(
        '@/modules/mcp/stores/McpToolCalls.store'
      )
      await holdPatch(() =>
        McpToolCalls.store.setState({
          error: 'Failed to load tool calls.',
          calls: [],
          loading: false,
        } as any),
      )
    },
  },
  // ── LlmModelsSection: models loading. The section early-returns unless a
  // REAL provider (from the loaded cassette) matches the route param, so pin the
  // param to the first enabled provider id and key llmModelsLoading to it. ─────
  {
    slug: 'seeded-llm-models-loading',
    title: 'LLM models section — loading',
    note: 'llmModelsLoading[providerId] → the <Loading/> block',
    path: '/gallery/:providerId',
    initialPath: `/gallery/${firstEnabledRemoteProviderId ?? llmProvidersList.providers[0]?.id ?? 'p1'}`,
    component: lazyNamed(
      () => import('@/modules/llm-provider/components/LlmModelsSection'),
      'LlmModelsSection',
    ),
    setup: async () => {
      const { LlmProviderStoreDef } = await import(
        '@/modules/llm-provider/stores/LlmProvider.store'
      )
      const pid =
        firstEnabledRemoteProviderId ?? llmProvidersList.providers[0]?.id ?? 'p1'
      await whenTrue(
        () => LlmProviderStoreDef.store.getState().providers.length > 0,
      )
      await holdPatch(() =>
        LlmProviderStoreDef.store.setState({
          llmModelsLoading: { [pid]: true },
        } as any),
      )
    },
  },
  // ── auth: ProviderButtons loading / error / empty (Stores.AuthProviders). ────
  {
    slug: 'seeded-provider-buttons-loading',
    title: 'OAuth provider buttons — loading',
    note: 'isLoading || !hasLoaded → the "Loading sign-in options" spinner',
    path: '/',
    initialPath: '/',
    component: lazyNamed(
      () => import('@/modules/auth/ProviderButtons'),
      'ProviderButtons',
    ),
    setup: async () => {
      const { AuthProviders } = await import('@/modules/auth/AuthProviders.store')
      await holdPatch(() =>
        AuthProviders.store.setState({ isLoading: true, hasLoaded: false } as any),
      )
    },
  },
  {
    slug: 'seeded-provider-buttons-error',
    title: 'OAuth provider buttons — error',
    note: 'error (loaded) → "Unable to load sign-in options" alert',
    path: '/',
    initialPath: '/',
    component: lazyNamed(
      () => import('@/modules/auth/ProviderButtons'),
      'ProviderButtons',
    ),
    setup: async () => {
      const { AuthProviders } = await import('@/modules/auth/AuthProviders.store')
      await holdPatch(() =>
        AuthProviders.store.setState({
          isLoading: false,
          hasLoaded: true,
          error: 'Unable to reach the sign-in service.',
          providers: [],
        } as any),
      )
    },
  },
  {
    slug: 'seeded-provider-buttons-empty',
    title: 'OAuth provider buttons — none configured',
    note: '!providers.length → renders nothing (no external sign-in)',
    path: '/',
    initialPath: '/',
    component: lazyNamed(
      () => import('@/modules/auth/ProviderButtons'),
      'ProviderButtons',
    ),
    setup: async () => {
      const { AuthProviders } = await import('@/modules/auth/AuthProviders.store')
      await holdPatch(() =>
        AuthProviders.store.setState({
          isLoading: false,
          hasLoaded: true,
          error: null,
          providers: [],
        } as any),
      )
    },
  },
  // ── auth: LoginForm / RegisterForm submit error (Stores.Auth.error). ─────────
  {
    slug: 'seeded-login-error',
    title: 'Login form — error',
    note: 'Stores.Auth.error → the login error alert',
    path: '/',
    initialPath: '/',
    component: lazyNamed(() => import('@/modules/auth/LoginForm'), 'LoginForm'),
    setup: async () => {
      const { Auth } = await import('@/modules/auth/Auth.store')
      await holdPatch(() =>
        Auth.store.setState({ error: 'Invalid email or password.' } as any),
      )
    },
  },
  {
    slug: 'seeded-register-error',
    title: 'Register form — error',
    note: 'Stores.Auth.error → the register error alert',
    path: '/',
    initialPath: '/',
    component: lazyNamed(
      () => import('@/modules/auth/RegisterForm'),
      'RegisterForm',
    ),
    setup: async () => {
      const { Auth } = await import('@/modules/auth/Auth.store')
      await holdPatch(() =>
        Auth.store.setState({ error: 'That email is already registered.' } as any),
      )
    },
  },
  // ── ChatMessage: a message with no content blocks → the `return null` arm. ───
  {
    slug: 'seeded-chat-message-empty',
    title: 'Chat message — no contents',
    note: '!message.contents || length===0 → renders nothing',
    path: '/',
    initialPath: '/',
    component: lazyProps(
      () => import('@/modules/chat/components/ChatMessage'),
      'ChatMessage',
      {
        message: {
          id: 'gallery-empty-msg',
          role: 'assistant',
          contents: [],
          originated_from_id: '',
          edit_count: 0,
          created_at: new Date().toISOString(),
          model_id: 'claude-opus-4-8',
        },
      },
    ),
  },
  // ── MessageList: a loaded conversation with zero messages → the empty state. ─
  {
    slug: 'seeded-message-list-empty',
    title: 'Message list — empty conversation',
    note: '!loading && messagesArray.length===0 → the empty conversation state',
    path: '/',
    initialPath: '/',
    component: lazyNamed(
      () => import('@/modules/chat/components/MessageList'),
      'MessageList',
    ),
    setup: async () => {
      const { useChatStore } = await import(
        '@/modules/chat/core/stores/Chat.store'
      )
      await holdPatch(() =>
        useChatStore.setState({
          messages: new Map(),
          loading: false,
          isStreaming: false,
        } as any),
      )
    },
  },
  // ── StepArtifacts: a step with no artifacts → the `return null` arm. ─────────
  {
    slug: 'seeded-step-artifacts-empty',
    title: 'Workflow step artifacts — empty',
    note: 'artifacts.length===0 → renders nothing',
    path: '/',
    initialPath: '/',
    component: lazyProps(
      () => import('@/modules/workflow/components/StepArtifacts'),
      'StepArtifacts',
      { runId: 'run-1', stepId: 'step-1', artifacts: [] },
    ),
  },
  // ── HardwareMonitor: no GPU devices → the "GPU Usage" empty card. currentUsage
  // arrives via the live hardware SSE (not a GET), so seed it on the store. ─────
  {
    slug: 'seeded-hardware-no-gpu',
    title: 'Hardware monitor — no GPU',
    note: '!currentUsage.gpu_devices.length → the GPU-empty card',
    path: '/',
    initialPath: '/',
    component: lazyNamed(
      () => import('@/modules/hardware/HardwareMonitor'),
      'HardwareMonitor',
    ),
    setup: async () => {
      const { Hardware } = await import('@/modules/hardware/Hardware.store')
      await holdPatch(() =>
        Hardware.store.setState({
          currentUsage: {
            cpu: { usage_percentage: 12 },
            memory: {
              available_ram: 8_000_000_000,
              used_ram: 8_000_000_000,
              usage_percentage: 50,
            },
            gpu_devices: [],
            timestamp: new Date().toISOString(),
          },
          usageLoading: false,
          usageError: null,
        } as any),
      )
    },
  },
  // ── ChatHistoryPage: the list-shown arm (conversations>0 || loading || error). ─
  {
    slug: 'seeded-chat-history-list',
    title: 'Chat history — list shown (loading)',
    note: 'loading && !isInitialized → the ConversationList load spinner (container mounted via the loading arm)',
    path: '/chat-history',
    initialPath: '/chat-history',
    // ChatHistoryPage is a DEFAULT export — `lazyNamed(…, 'ChatHistoryPage')`
    // resolved to `undefined` (blank via the boundary). Load the default.
    component: lazyNamed(
      () => import('@/modules/chat/pages/ChatHistoryPage'),
      'default',
    ),
    setup: async () => {
      const { ChatHistory } = await import(
        '@/modules/chat/stores/ChatHistory.store'
      )
      const { AppLayout } = await import(
        '@/modules/layouts/app-layout/AppLayout.store'
      )
      // ChatHistoryPage refetches on mount (which flips loading/isInitialized as
      // it resolves), so a one-shot seed races into a blank window: `loading`
      // (mid-fetch) with a seeded `isInitialized:true` matches NEITHER the error
      // arm (needs !loading) NOR the spinner arm (needs !isInitialized) → blank.
      // Assert a persistent loading state (`holdForever`) so the container mounts
      // via the loading arm (also covering the `nativeScroll===true` :143 ternary)
      // and ConversationList deterministically shows its load spinner.
      holdForever(() => {
        AppLayout.store.setState({ nativeScroll: true } as any)
        ChatHistory.store.setState({
          loading: true,
          isInitialized: false,
          conversations: [],
          error: null,
        } as any)
      })
    },
  },
  // ── SHADOW: /settings landing. The enumerated `/settings` route's element is
  // `() => null` — its real content (the settings nav + a redirect to the first
  // section) lives in `SettingsLayoutDef`, which the page grid doesn't apply, so
  // the enumerated surface is blank. Render `SettingsPage` (the nav shell) inside
  // its own router landed on a section so the genuine settings landing chrome is
  // reviewable. ────────────────────────────────────────────────────────────────
  {
    slug: 'settings',
    title: 'Settings landing (nav shell)',
    note: '/settings redirects to the first section via SettingsLayout; the page grid renders the null index element. This renders SettingsPage on the first section so the real settings nav chrome is reviewable.',
    // Mount SettingsPage under the frame's OWN router (no nested MemoryRouter —
    // React Router forbids that) at a concrete section so its redirect effect is
    // a no-op and the nav menu + header render. The section Outlet has no child
    // route (each section is reviewed as its own enumerated surface), so the
    // content area is intentionally empty — the point is the nav shell.
    path: '/settings/:section',
    initialPath: '/settings/general',
    component: lazyNamed(
      () => import('@/modules/settings/SettingsPage'),
      'default',
    ),
  },
  // ── SHADOW: /hardware-monitor live metrics. Usage data arrives over the
  // `/api/hardware/stream` SSE connection, not a JSON GET, so the GET-only loaded
  // cassette leaves `currentUsage` null → "Waiting for usage data…". Seed a
  // realistic snapshot (CPU/mem/GPU) so the charts render. ──────────────────────
  {
    slug: 'hardware-monitor',
    title: 'Hardware monitor — live metrics',
    note: 'currentUsage arrives over SSE (not a JSON GET); seed a realistic usage snapshot so the CPU/memory/GPU charts render instead of "Waiting for usage data…".',
    path: '/hardware-monitor',
    initialPath: '/hardware-monitor',
    component: lazyNamed(
      () => import('@/modules/hardware/HardwareMonitor'),
      'HardwareMonitor',
    ),
    setup: async () => {
      const { Hardware } = await import('@/modules/hardware/Hardware.store')
      await holdPatch(() =>
        Hardware.store.setState({
          hardwareInfo: {
            cpu: {
              architecture: 'x86_64',
              model: 'AMD Ryzen 9 7950X',
              cores: 16,
              threads: 32,
              base_frequency: 4500,
              max_frequency: 5700,
            },
            gpu_devices: [
              {
                device_id: 'gpu-0',
                name: 'NVIDIA GeForce RTX 4090',
                vendor: 'NVIDIA',
                memory: 25757220864,
                driver_version: '550.90.07',
                compute_capabilities: {} as any,
              },
            ],
            memory: { total_ram: 68719476736, total_swap: 8589934592 },
            operating_system: {
              architecture: 'x86_64',
              name: 'Linux',
              version: '24.04',
              kernel_version: '6.8.0',
            },
          },
          hardwareInitialized: true,
          hardwareLoading: false,
          hardwareError: null,
          currentUsage: {
            cpu: { usage_percentage: 37.4, temperature: 58, frequency: 4820 },
            gpu_devices: [
              {
                device_id: 'gpu-0',
                device_name: 'NVIDIA GeForce RTX 4090',
                utilization_percentage: 72,
                memory_total: 25757220864,
                memory_used: 14200000000,
                memory_usage_percentage: 55.1,
                temperature: 64,
                power_usage: 285,
              },
            ],
            memory: {
              total_ram: 68719476736,
              used_ram: 28051503104,
              available_ram: 40667973632,
              usage_percentage: 40.8,
              used_swap: 0,
              available_swap: 8589934592,
            } as any,
            timestamp: new Date().toISOString(),
          },
          usageLoading: false,
          usageError: null,
          sseConnected: true,
          sseConnecting: false,
          sseError: null,
        } as any),
      )
    },
  },
  // ── /hardware-monitor cold-load ERROR. The metrics shadow above seeds a full
  // snapshot so it can only ever show the loaded charts; the error branch
  // (`hardwareError && !hardwareInfo` → ErrorState) is otherwise unreachable in
  // the gallery because the shadow owns the `hardware-monitor` slug. Seed a
  // load failure (no hardwareInfo) so the real ErrorState is reviewable. ───────
  {
    slug: 'seeded-hardware-monitor-error',
    title: 'Hardware monitor — load error',
    note: 'hardwareError && !hardwareInfo → the in-place "Couldn\'t load hardware monitor" ErrorState (the cold hardware-info GET failed).',
    path: '/hardware-monitor',
    initialPath: '/hardware-monitor',
    component: lazyNamed(
      () => import('@/modules/hardware/HardwareMonitor'),
      'HardwareMonitor',
    ),
    setup: async () => {
      const { Hardware } = await import('@/modules/hardware/Hardware.store')
      // holdPatch re-asserts the failure so the store's init loadHardwareInfo()
      // (which succeeds against the loaded cassette) can't clobber it back to a
      // healthy state.
      await holdPatch(() =>
        Hardware.store.setState({
          hardwareInfo: null,
          hardwareInitialized: false,
          hardwareLoading: false,
          hardwareError: 'Internal server error',
          currentUsage: null,
          usageLoading: false,
          sseConnected: false,
          sseConnecting: false,
        } as any),
      )
    },
  },
  // ── SHADOW: /auth/link-account form. The page shows a "Missing link token"
  // error banner whenever `?link_token=` is absent — which is EVERY enumerated
  // state (the route carries no token), so the review saw the error banner
  // mislabeled as the "empty" state. Mount it WITH a token so the real link-your-
  // accounts form renders; the missing-token banner is a genuinely separate
  // state, not "empty". ────────────────────────────────────────────────────────
  {
    slug: 'auth-link-account',
    title: 'Link account — password confirm form',
    note: 'the page shows a missing-token error banner without ?link_token=; mount with a token so the real form renders (the banner is a separate state, not "empty").',
    path: '/auth/link-account',
    initialPath: '/auth/link-account?link_token=gallery-demo-link-token',
    component: lazyNamed(
      () => import('@/modules/auth/LinkAccountPage'),
      'LinkAccountPage',
    ),
  },
]

export const SEEDED_SURFACE_ENTRIES: SeededSurfaceEntry[] = [
  ...integratorSeeded,
  ...shard1Seeded,
  ...shard2Seeded,
  ...shard3Seeded,
  ...shard4Seeded,
  ...shard5Seeded,
]

export const seededSurfaceBySlug = (slug: string) =>
  SEEDED_SURFACE_ENTRIES.find(e => e.slug === slug)

export const SEEDED_SURFACE_SLUGS = SEEDED_SURFACE_ENTRIES.map(e => e.slug)

const seededTestId = (slug: string) => `gallery-page-${slug}`

/** Renders one seeded-surface entry: the real component + a mount-time store seed. */
export function SeededSurfaceFrame({
  entry,
}: {
  entry: SeededSurfaceEntry
}): ReactNode {
  useEffect(() => {
    void entry.setup?.()
  }, [entry])
  useRunInteraction(entry.interactions, 1200)
  const Component = entry.component
  return (
    <section
      data-testid={seededTestId(entry.slug)}
      data-gallery-state="seeded"
      className="flex flex-col gap-3 border border-border rounded-lg p-4 bg-background"
    >
      <div className="flex flex-col gap-1" data-gallery-chrome>
        <Title level={3}>
          {entry.title}
          <Text tone="muted" className="ml-2 text-sm">
            · seeded
          </Text>
        </Title>
        <Text tone="muted" className="text-sm">
          gallery-page-{entry.slug} · {entry.note}
        </Text>
      </div>
      <div
        className="w-full overflow-hidden rounded-md border border-border bg-background"
        style={{ height: 720 }}
      >
        <AppErrorBoundary label={`seeded-${entry.slug}`} fallback={() => null}>
          <MemoryRouter initialEntries={[entry.initialPath]}>
            <Routes>
              <Route
                path={entry.path}
                element={
                  <Suspense fallback={<Loading />}>
                    <Component />
                  </Suspense>
                }
              />
            </Routes>
          </MemoryRouter>
        </AppErrorBoundary>
      </div>
    </section>
  )
}
