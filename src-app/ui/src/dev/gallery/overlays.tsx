/**
 * Overlay open-state entries — drawers/dialogs/menus rendered in their OPEN
 * state with seeded data. Each entry seeds + fires the overlay's store open
 * action on mount, then renders the component; the Base-UI Sheet/Dialog portals
 * to the body, so a full-page screenshot captures it.
 *
 * Rendered ONE-PER-PAGE-LOAD via the URL-isolation path (`?surface=<slug>&state=open`)
 * — like chat detail — so the per-overlay singleton store never bleeds across
 * entries and multiple open portals don't stack on one canvas.
 */
import { type ComponentType, type LazyExoticComponent, lazy } from 'react'
import { Stores } from '@/core/stores'
import { dialog } from '@/components/ui'
import { adminUser } from './fixtures/auth'
import { llmProvidersList, llmGroupsList } from './fixtures/llm-providers'
import { deepProjectFiles } from './fixtures/project-deep'
import {
  SKILLS_CONVERSATION_ID,
  skillsAvailable,
  skillsList,
} from './fixtures/skills'
import { holdPatch } from './seeded/helpers'
import type { InteractionRecipe } from './interactions'

export interface OverlayEntry {
  /** Gallery slug → `?surface=<slug>&state=open`; also the section testid. */
  slug: string
  /** Coverage surface id (the component file). */
  surface: string
  /** Human title for the frame. */
  title: string
  component: LazyExoticComponent<ComponentType>
  /** Seed + fire the store open action (runs on mount). Optional: prop-driven
   *  overlays render open via bound props (see `lazyBound`) with no store call. */
  open?: () => void | Promise<void>
  /** Interaction recipes driven after the overlay opens (focus an input, submit
   *  invalid, …). Driven via `?surface=<slug>&interact=<name>`. */
  interactions?: InteractionRecipe[]
}

const provider = llmProvidersList.providers[0]
const group = llmGroupsList.groups[0]

const lazyNamed = (loader: () => Promise<any>, name: string) =>
  lazy(() => loader().then(m => ({ default: m[name] })))

/** Lazy-load a named export and render it with fixed props — the prop-driven
 *  analog of `lazyNamed` for overlays whose visibility is a parent-passed `open`
 *  prop (not a store). Props are cast (dev-only gallery fixtures). */
const lazyBound = (
  loader: () => Promise<any>,
  name: string,
  props: Record<string, unknown>,
) =>
  lazy(async () => {
    const C = (await loader())[name] as ComponentType<any>
    return { default: () => <C {...(props as any)} /> }
  })

const noop = () => {}

// ── Minimal inline fixtures for prop/entity-driven overlays (dev-only). ────────
const fileFixture = deepProjectFiles[0]

const workflowFixture = {
  id: 'wf-gallery-0001',
  name: 'Weekly literature digest',
  description: 'Search, screen, and summarize new papers on a saved query.',
  scope: 'user',
  version: '1.0.0',
  is_system: false,
  enabled: true,
  created_at: '2026-02-01T10:00:00Z',
  updated_at: '2026-02-01T10:00:00Z',
  compiled_ir_json: {
    inputs: [
      { name: 'query', description: 'Search terms', required: true },
      { name: 'max_results', description: 'Cap', required: false, default: 20 },
    ],
    steps: [{ id: 'search' }, { id: 'summarize' }],
  },
} as const

const hubModelFixture = {
  id: 'hub-model-0001',
  name: 'qwen2.5-coder-7b-instruct',
  display_name: 'Qwen2.5 Coder 7B Instruct',
  description: 'A compact coding model, GGUF quantized.',
  author: 'Qwen',
  downloads: 128_000,
  tags: ['code', 'gguf'],
} as const

const hubMcpFixture = {
  id: 'hub-mcp-0001',
  name: 'filesystem',
  display_name: 'Filesystem MCP',
  description: 'Read/write files under an allowed root.',
  author: 'modelcontextprotocol',
  tags: ['files'],
} as const

const hubAssistantFixture = {
  id: 'hub-asst-0001',
  name: 'research-assistant',
  display_name: 'Research Assistant',
  description: 'A methodical literature-review companion.',
  author: 'ziee',
  tags: ['research'],
} as const

const hubSkillFixture = {
  id: 'hub-skill-0001',
  name: 'com.ziee.pdf-forms',
  display_name: 'PDF form filling',
  description: 'Fill and extract PDF form fields.',
  author: 'ziee',
  tags: ['pdf'],
} as const

const hubWorkflowFixture = {
  id: 'hub-wf-0001',
  name: 'systematic-review',
  display_name: 'Systematic review',
  description: 'PRISMA-style screening pipeline.',
  author: 'ziee',
  tags: ['research'],
} as const

/** Seed the install list + per-conversation available set through the REAL
 *  stores (holdPatch re-asserts over any late mock-API load) so the Skills
 *  dialog renders populated / empty. */
async function seedSkills(
  skills: typeof skillsList,
  available: typeof skillsAvailable,
): Promise<void> {
  const { SkillStoreDef } = await import('@/modules/skill/stores/Skill.store')
  const { ConversationSkills } = await import(
    '@/modules/skill/stores/ConversationSkills.store'
  )
  await holdPatch(() => {
    SkillStoreDef.store.setState({ skills, loading: false } as any)
    ConversationSkills.store.setState({
      available: { [SKILLS_CONVERSATION_ID]: available },
      loading: { [SKILLS_CONVERSATION_ID]: false },
      error: null,
    } as any)
  })
}

/** Seed the loading arm (available undefined + loading true). */
async function seedSkillsLoading(): Promise<void> {
  const { ConversationSkills } = await import(
    '@/modules/skill/stores/ConversationSkills.store'
  )
  await holdPatch(() =>
    ConversationSkills.store.setState({
      available: {},
      loading: { [SKILLS_CONVERSATION_ID]: true },
      error: null,
    } as any),
  )
}

export const OVERLAY_ENTRIES: OverlayEntry[] = [
  {
    slug: 'overlay-llm-provider-drawer',
    surface: 'modules/llm-provider/components/LlmProviderDrawer',
    title: 'Edit LLM Provider (drawer)',
    component: lazyNamed(
      () => import('@/modules/llm-provider/components/LlmProviderDrawer'),
      'LlmProviderDrawer',
    ),
    open: () => Stores.LlmProviderDrawer.openLlmProviderDrawer(provider),
  },
  {
    slug: 'overlay-create-user-drawer',
    surface: 'modules/user/components/user/CreateUserDrawer',
    title: 'Create User (drawer)',
    component: lazyNamed(
      () => import('@/modules/user/components/user/CreateUserDrawer'),
      'CreateUserDrawer',
    ),
    open: () => Stores.CreateUserDrawer.openCreateUserDrawer(),
    interactions: [
      {
        name: 'focus-input',
        note: 'focus the username field → the :focus-visible ring (drives G7: ring clipping / offset in a dense drawer form)',
        steps: async d => {
          await d.focus('user-create-username-input')
        },
      },
      {
        name: 'submit-invalid',
        note: 'submit the empty form → inline required-field validation (G6 error state per input)',
        steps: async d => {
          await d.click('user-create-submit-button')
          await d.wait(300)
        },
      },
    ],
  },
  {
    slug: 'overlay-edit-user-drawer',
    surface: 'modules/user/components/user/EditUserDrawer',
    title: 'Edit User (drawer)',
    component: lazyNamed(
      () => import('@/modules/user/components/user/EditUserDrawer'),
      'EditUserDrawer',
    ),
    open: () => Stores.EditUserDrawer.openEditUserDrawer(adminUser),
  },
  {
    slug: 'overlay-reset-password-drawer',
    surface: 'modules/user/components/user/ResetPasswordDrawer',
    title: 'Reset Password (drawer)',
    component: lazyNamed(
      () => import('@/modules/user/components/user/ResetPasswordDrawer'),
      'ResetPasswordDrawer',
    ),
    open: () => Stores.ResetPasswordDrawer.openResetPasswordDrawer(adminUser),
  },
  {
    slug: 'overlay-edit-user-group-drawer',
    surface: 'modules/user/components/group/EditUserGroupDrawer',
    title: 'Edit User Group (drawer)',
    component: lazyNamed(
      () => import('@/modules/user/components/group/EditUserGroupDrawer'),
      'EditUserGroupDrawer',
    ),
    open: () => Stores.EditUserGroupDrawer.openUserGroupDrawer(group),
  },
  {
    slug: 'overlay-assign-group-drawer',
    surface: 'modules/user/components/user/AssignGroupDrawer',
    title: 'Assign Group (drawer)',
    component: lazyNamed(
      () => import('@/modules/user/components/user/AssignGroupDrawer'),
      'AssignGroupDrawer',
    ),
    open: () => Stores.AssignGroupDrawer.openAssignGroupDrawer(adminUser),
    interactions: [
      {
        name: 'submit-empty',
        note: 'submit with no group selected → the handleAssignGroup empty-selection guard (AssignGroupDrawer.tsx:63)',
        steps: async d => {
          await d.click('user-assign-group-submit-button')
          await d.wait(200)
        },
      },
    ],
  },
  {
    slug: 'overlay-user-groups-drawer',
    surface: 'modules/user/components/user/UserGroupsDrawer',
    title: 'User Groups (drawer)',
    component: lazyNamed(
      () => import('@/modules/user/components/user/UserGroupsDrawer'),
      'UserGroupsDrawer',
    ),
    open: () => Stores.UserGroupsDrawer.openUserGroupsDrawer(adminUser),
  },
  {
    slug: 'overlay-group-members-drawer',
    surface: 'modules/user/components/group/GroupMembersDrawer',
    title: 'Group Members (drawer)',
    component: lazyNamed(
      () => import('@/modules/user/components/group/GroupMembersDrawer'),
      'GroupMembersDrawer',
    ),
    open: () => Stores.GroupMembersDrawer.openGroupMembersDrawer(group),
  },
  {
    slug: 'overlay-llm-repository-drawer',
    surface: 'modules/llm-repository/components/LlmRepositoryDrawer',
    title: 'LLM Repository (drawer)',
    component: lazyNamed(
      () => import('@/modules/llm-repository/components/LlmRepositoryDrawer'),
      'LlmRepositoryDrawer',
    ),
    open: () => Stores.LlmRepositoryDrawer.openDrawer(),
  },
  {
    slug: 'overlay-group-llm-providers-assignment',
    surface: 'modules/llm-provider/components/GroupLlmProvidersAssignmentDrawer',
    title: 'Group → LLM Providers (drawer)',
    component: lazyNamed(
      () => import('@/modules/llm-provider/components/GroupLlmProvidersAssignmentDrawer'),
      'GroupLlmProvidersAssignmentDrawer',
    ),
    open: () => Stores.GroupLlmProvidersAssignment.openDrawer(group),
  },
  {
    slug: 'overlay-group-mcp-servers-assignment',
    surface: 'modules/mcp/components/system/GroupSystemMcpServersAssignmentDrawer',
    title: 'Group → MCP Servers (drawer)',
    component: lazyNamed(
      () => import('@/modules/mcp/components/system/GroupSystemMcpServersAssignmentDrawer'),
      'GroupSystemMcpServersAssignmentDrawer',
    ),
    open: () => Stores.GroupSystemMcpServersAssignment.openDrawer(group),
  },
  {
    slug: 'overlay-group-skills-assignment',
    surface: 'modules/skill/widgets/GroupSystemSkillsAssignmentDrawer',
    title: 'Group → Skills (drawer)',
    component: lazyNamed(
      () => import('@/modules/skill/widgets/GroupSystemSkillsAssignmentDrawer'),
      'GroupSystemSkillsAssignmentDrawer',
    ),
    open: () => Stores.GroupSystemSkillsAssignment.openDrawer(group),
  },
  {
    slug: 'overlay-group-workflows-assignment',
    surface: 'modules/workflow/widgets/GroupSystemWorkflowsAssignmentDrawer',
    title: 'Group → Workflows (drawer)',
    component: lazyNamed(
      () => import('@/modules/workflow/widgets/GroupSystemWorkflowsAssignmentDrawer'),
      'GroupSystemWorkflowsAssignmentDrawer',
    ),
    open: () => Stores.GroupSystemWorkflowsAssignment.openDrawer(group),
  },
  {
    slug: 'overlay-assistant-form-drawer',
    surface: 'modules/assistant/components/AssistantFormDrawer',
    title: 'Create Assistant (drawer)',
    component: lazyNamed(
      () => import('@/modules/assistant/components/AssistantFormDrawer'),
      'AssistantFormDrawer',
    ),
    open: () => Stores.AssistantDrawer.openAssistantDrawer(),
  },

  // ══════════════════════════════════════════════════════════════════════════
  // OVERLAY-RENDER SWEEP — every remaining Dialog/Drawer/Sheet host rendered
  // OPEN with seeded data, so geometry/affordance/runtime/vision audits can
  // finally SEE them. (Previously ~0 of these were ever on screen.)
  // ══════════════════════════════════════════════════════════════════════════

  // ── PRIORITY #1: "Skills in this conversation" dialog — populated / empty /
  //    loading. The full Dialog chrome + populated panel + nested detail drawer,
  //    which no audit had ever rendered. ────────────────────────────────────────
  {
    slug: 'overlay-skills-conversation-loaded',
    surface: 'modules/skill/components/SkillConversationDrawer',
    title: 'Skills in this conversation — populated',
    component: lazyBound(
      () => import('@/modules/skill/components/SkillConversationDrawer'),
      'SkillConversationDrawer',
      { conversationId: SKILLS_CONVERSATION_ID },
    ),
    open: () => {
      Stores.SkillConversationDrawer.openDrawer()
      void seedSkills(skillsList, skillsAvailable)
    },
    interactions: [
      {
        name: 'open-detail',
        note: 'click a skill row → the nested SkillDetailDrawer opens WITH conversationId (the "Hide in this conversation" checkbox path)',
        steps: async d => {
          await d.click(`skill-conversation-open-${skillsList[0].id}`)
          await d.wait(400)
        },
      },
    ],
  },
  {
    slug: 'overlay-skills-conversation-empty',
    surface: 'modules/skill/components/SkillConversationDrawer',
    title: 'Skills in this conversation — empty',
    component: lazyBound(
      () => import('@/modules/skill/components/SkillConversationDrawer'),
      'SkillConversationDrawer',
      { conversationId: SKILLS_CONVERSATION_ID },
    ),
    open: () => {
      Stores.SkillConversationDrawer.openDrawer()
      void seedSkills([], [])
    },
  },
  {
    slug: 'overlay-skills-conversation-loading',
    surface: 'modules/skill/components/SkillConversationDrawer',
    title: 'Skills in this conversation — loading',
    component: lazyBound(
      () => import('@/modules/skill/components/SkillConversationDrawer'),
      'SkillConversationDrawer',
      { conversationId: SKILLS_CONVERSATION_ID },
    ),
    open: () => {
      Stores.SkillConversationDrawer.openDrawer()
      void seedSkillsLoading()
    },
  },
  {
    slug: 'overlay-skill-detail-drawer',
    surface: 'modules/skill/components/SkillDetailDrawer',
    title: 'Skill detail (drawer) — with conversation hide-toggle',
    component: lazyNamed(
      () => import('@/modules/skill/components/SkillDetailDrawer'),
      'SkillDetailDrawer',
    ),
    open: () => {
      void seedSkills(skillsList, skillsAvailable)
      Stores.SkillDrawer.open(skillsList[0] as any, SKILLS_CONVERSATION_ID)
    },
  },
  {
    slug: 'overlay-import-skill-dialog',
    surface: 'modules/skill/components/ImportSkillDialog',
    title: 'Import skill (dialog)',
    component: lazyBound(
      () => import('@/modules/skill/components/ImportSkillDialog'),
      'ImportSkillDialog',
      { open: true, onClose: noop },
    ),
  },

  // ── File preview drawer ──────────────────────────────────────────────────────
  {
    slug: 'overlay-file-preview-drawer',
    surface: 'modules/file/components/FilePreviewDrawer',
    title: 'File preview (drawer)',
    component: lazyNamed(
      () => import('@/modules/file/components/FilePreviewDrawer'),
      'FilePreviewDrawer',
    ),
    open: () => Stores.FilePreviewDrawer.openPreview(fileFixture as any),
  },

  // ── MCP server drawer (edit) + config modal ──────────────────────────────────
  {
    slug: 'overlay-mcp-server-drawer',
    surface: 'modules/mcp/components/common/McpServerDrawer',
    title: 'MCP server (drawer, create)',
    component: lazyNamed(
      () => import('@/modules/mcp/components/common/McpServerDrawer'),
      'McpServerDrawer',
    ),
    open: () => Stores.McpServerDrawer.openMcpServerDrawer(),
  },
  {
    slug: 'overlay-mcp-config-modal',
    surface: 'modules/mcp/components/McpConfigModal',
    title: 'MCP config (modal)',
    component: lazyNamed(
      () => import('@/modules/mcp/components/McpConfigModal'),
      'McpConfigModal',
    ),
    open: () => Stores.McpComposer.openConfigModal(),
  },

  // ── Project form drawer (create) + add-to-project modal ──────────────────────
  {
    slug: 'overlay-project-form-drawer',
    surface: 'modules/projects/components/ProjectFormDrawer',
    title: 'Create project (drawer)',
    component: lazyNamed(
      () => import('@/modules/projects/components/ProjectFormDrawer'),
      'ProjectFormDrawer',
    ),
    open: () => Stores.ProjectDrawer.openProjectDrawer(),
  },
  {
    slug: 'overlay-add-to-project-modal',
    surface: 'modules/projects/components/AddToProjectModal',
    title: 'Add conversation to project (modal)',
    component: lazyBound(
      () => import('@/modules/projects/components/AddToProjectModal'),
      'AddToProjectModal',
      { open: true, conversationId: 'conv-1', onClose: noop },
    ),
  },

  // ── LLM model drawers ────────────────────────────────────────────────────────
  {
    slug: 'overlay-edit-llm-model-drawer',
    surface: 'modules/llm-provider/components/llm-models/EditLlmModelDrawer',
    title: 'Edit LLM model (drawer)',
    component: lazyNamed(
      () => import('@/modules/llm-provider/components/llm-models/EditLlmModelDrawer'),
      'EditLlmModelDrawer',
    ),
    open: () =>
      Stores.EditLlmModelDrawer.openEditLlmModelDrawer(
        (llmProvidersList.providers[0] as any)?.id ?? 'model-1',
      ),
  },
  {
    slug: 'overlay-add-remote-llm-model-drawer',
    surface: 'modules/llm-provider/components/llm-models/AddRemoteLlmModelDrawer',
    title: 'Add remote LLM model (drawer)',
    component: lazyNamed(
      () => import('@/modules/llm-provider/components/llm-models/AddRemoteLlmModelDrawer'),
      'AddRemoteLlmModelDrawer',
    ),
    open: () =>
      Stores.AddRemoteLlmModelDrawer.openAddRemoteLlmModelDrawer(
        provider.id,
        (provider as any).provider_type ?? 'openai',
      ),
  },
  {
    slug: 'overlay-add-local-llm-model-upload-drawer',
    surface: 'modules/llm-provider/components/llm-models/AddLocalLlmModelUploadDrawer',
    title: 'Add local LLM model — upload (drawer)',
    component: lazyNamed(
      () =>
        import('@/modules/llm-provider/components/llm-models/AddLocalLlmModelUploadDrawer'),
      'AddLocalLlmModelUploadDrawer',
    ),
    open: () =>
      Stores.AddLocalLlmModelUploadDrawer.openAddLocalLlmModelUploadDrawer(provider.id),
  },
  {
    slug: 'overlay-add-local-llm-model-download-drawer',
    surface:
      'modules/llm-provider/components/llm-models/AddLocalLlmModelDownloadDrawer',
    title: 'Add local LLM model — download (drawer)',
    component: lazyNamed(
      () =>
        import('@/modules/llm-provider/components/llm-models/AddLocalLlmModelDownloadDrawer'),
      'AddLocalLlmModelDownloadDrawer',
    ),
    open: () =>
      Stores.AddLocalLlmModelDownloadDrawer.openAddLocalLlmModelDownloadDrawer(
        provider.id,
      ),
  },

  // ── Runtime download drawer ──────────────────────────────────────────────────
  {
    slug: 'overlay-runtime-download-drawer',
    surface: 'modules/llm-local-runtime/components/drawers/RuntimeDownloadDrawer',
    title: 'Runtime engine download (drawer)',
    component: lazyNamed(
      () =>
        import('@/modules/llm-local-runtime/components/drawers/RuntimeDownloadDrawer'),
      'RuntimeDownloadDrawer',
    ),
    open: () =>
      Stores.RuntimeDownloadDrawer.openDrawer({
        id: 'llamacpp',
        name: 'llama.cpp',
        display_name: 'llama.cpp',
      } as any),
  },

  // ── Auth provider edit drawer + provider API-key modal ───────────────────────
  {
    slug: 'overlay-auth-provider-edit-drawer',
    surface: 'modules/auth-providers/components/AuthProviderEditDrawer',
    title: 'Edit auth provider (drawer)',
    component: lazyBound(
      () => import('@/modules/auth-providers/components/AuthProviderEditDrawer'),
      'AuthProviderEditDrawer',
      { open: true, onClose: noop },
    ),
  },
  {
    slug: 'overlay-provider-api-key-modal',
    surface: 'modules/user-llm-providers/chat-extension/components/ProviderApiKeyModal',
    title: 'Provider API key (modal)',
    component: lazyBound(
      () =>
        import(
          '@/modules/user-llm-providers/chat-extension/components/ProviderApiKeyModal'
        ),
      'ProviderApiKeyModal',
      {
        providerId: provider.id,
        providerName: (provider as any).name ?? 'OpenAI',
        modelId: 'model-1',
        onSuccess: noop,
        onCancel: noop,
      },
    ),
  },

  // ── Citations import modal ───────────────────────────────────────────────────
  {
    slug: 'overlay-import-citations-modal',
    surface: 'modules/citations/components/ImportCitationsModal',
    title: 'Import citations (modal)',
    component: lazyBound(
      () => import('@/modules/citations/components/ImportCitationsModal'),
      'ImportCitationsModal',
      { open: true, onClose: noop, projectId: null },
    ),
  },

  // ── Workflow dialogs + detail drawer ─────────────────────────────────────────
  {
    slug: 'overlay-workflow-detail-drawer',
    surface: 'modules/workflow/components/WorkflowDetailDrawer',
    title: 'Workflow detail (drawer)',
    component: lazyNamed(
      () => import('@/modules/workflow/components/WorkflowDetailDrawer'),
      'WorkflowDetailDrawer',
    ),
    open: () => Stores.WorkflowDrawer.open(workflowFixture as any),
  },
  {
    slug: 'overlay-import-workflow-dialog',
    surface: 'modules/workflow/components/ImportWorkflowDialog',
    title: 'Import workflow (dialog)',
    component: lazyBound(
      () => import('@/modules/workflow/components/ImportWorkflowDialog'),
      'ImportWorkflowDialog',
      { open: true, onClose: noop },
    ),
  },
  {
    slug: 'overlay-workflow-run-dialog',
    surface: 'modules/workflow/components/WorkflowRunDialog',
    title: 'Run workflow (dialog)',
    component: lazyBound(
      () => import('@/modules/workflow/components/WorkflowRunDialog'),
      'WorkflowRunDialog',
      {
        open: true,
        onClose: noop,
        conversationId: 'conv-1',
        workflow: workflowFixture,
        onStarted: noop,
      },
    ),
  },
  {
    slug: 'overlay-dry-run-preview-dialog',
    surface: 'modules/workflow/components/DryRunPreviewDialog',
    title: 'Dry-run preview (dialog)',
    component: lazyBound(
      () => import('@/modules/workflow/components/DryRunPreviewDialog'),
      'DryRunPreviewDialog',
      { open: true, onClose: noop, workflow: workflowFixture },
    ),
  },
  {
    slug: 'overlay-workflow-tests-panel',
    surface: 'modules/workflow/components/WorkflowTestsPanel',
    title: 'Workflow tests (dialog)',
    component: lazyBound(
      () => import('@/modules/workflow/components/WorkflowTestsPanel'),
      'WorkflowTestsPanel',
      { open: true, onClose: noop, workflow: workflowFixture },
    ),
  },

  // ── Hub detail drawers (assistants / models / mcp / skills / workflows) ───────
  {
    slug: 'overlay-hub-assistant-details-drawer',
    surface: 'modules/hub/modules/assistants/components/AssistantDetailsDrawer',
    title: 'Hub assistant details (drawer)',
    component: lazyBound(
      () => import('@/modules/hub/modules/assistants/components/AssistantDetailsDrawer'),
      'AssistantDetailsDrawer',
      { open: true, onClose: noop, assistant: hubAssistantFixture },
    ),
  },
  {
    slug: 'overlay-hub-model-details-drawer',
    surface: 'modules/hub/modules/llm-models/components/ModelDetailsDrawer',
    title: 'Hub model details (drawer)',
    component: lazyNamed(
      () => import('@/modules/hub/modules/llm-models/components/ModelDetailsDrawer'),
      'ModelDetailsDrawer',
    ),
    open: () => Stores.ModelDetailsDrawer.open(hubModelFixture as any),
  },
  {
    slug: 'overlay-hub-mcp-details-drawer',
    surface: 'modules/hub/modules/mcp/components/McpServerDetailsDrawer',
    title: 'Hub MCP server details (drawer)',
    component: lazyNamed(
      () => import('@/modules/hub/modules/mcp/components/McpServerDetailsDrawer'),
      'McpServerDetailsDrawer',
    ),
    open: () => Stores.McpServerDetailsDrawer.open(hubMcpFixture as any),
  },
  {
    slug: 'overlay-hub-skill-details-drawer',
    surface: 'modules/hub/modules/skill/components/SkillDetailsDrawer',
    title: 'Hub skill details (drawer)',
    component: lazyBound(
      () => import('@/modules/hub/modules/skill/components/SkillDetailsDrawer'),
      'SkillDetailsDrawer',
      { open: true, onClose: noop, item: hubSkillFixture },
    ),
  },
  {
    slug: 'overlay-hub-workflow-details-drawer',
    surface: 'modules/hub/modules/workflow/components/WorkflowDetailsDrawer',
    title: 'Hub workflow details (drawer)',
    component: lazyBound(
      () => import('@/modules/hub/modules/workflow/components/WorkflowDetailsDrawer'),
      'WorkflowDetailsDrawer',
      { open: true, onClose: noop, item: hubWorkflowFixture },
    ),
  },

  {
    // <DialogHost/> singleton with a DESCRIBED alert → the open AlertDialog (:94)
    // + the `description != null` arm of the aria-describedby spread (:95).
    slug: 'overlay-dialog-host-described',
    surface: 'components/ui/kit/dialog-host',
    title: 'Imperative dialog — described',
    component: lazyNamed(() => import('@/components/ui'), 'DialogHost'),
    open: () => {
      void dialog.info({
        title: 'Heads up',
        description: 'A described alert dialog.',
        okText: 'OK',
        testid: 'gallery-dialog-with-desc',
      })
    },
  },
  {
    // Bare alert (no description) → the `description == null` arm of :95
    // (aria-describedby explicitly undefined). Separate frame: two simultaneously
    // -open Radix AlertDialogs don't both mount.
    slug: 'overlay-dialog-host-bare',
    surface: 'components/ui/kit/dialog-host',
    title: 'Imperative dialog — bare (no description)',
    component: lazyNamed(() => import('@/components/ui'), 'DialogHost'),
    open: () => {
      void dialog.warning({
        title: 'Bare alert (no description)',
        okText: 'OK',
        testid: 'gallery-dialog-no-desc',
      })
    },
  },
]

/** Surface ids covered with a delivered open-state entry (for the coverage gate). */
export const WIRED_OVERLAY_SURFACES = new Set(OVERLAY_ENTRIES.map(o => o.surface))

export const overlayBySlug = (slug: string) =>
  OVERLAY_ENTRIES.find(o => o.slug === slug)
