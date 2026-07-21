/**
 * Dev-gallery seed for the `hub` module — the per-kind hub detail drawers
 * (assistants / models / mcp / skills / workflows), rendered OPEN with inline
 * fixtures. Auto-discovered by the gallery's runtime registry
 * (`@/dev/gallery/support`); never imported by `module.tsx`, so it is dev-only
 * and tree-shaken from prod.
 */
import type { ModuleGallery } from '@/dev/gallery/support'
import { lazyBound, lazyNamed } from '@/dev/gallery/support'
import { ModelDetailsDrawer as ModelDetailsDrawerStore } from '@/modules/hub/modules/llm-models/components/modelDetailsDrawer'
import { McpServerDetailsDrawer as McpServerDetailsDrawerStore } from '@/modules/hub/modules/mcp/components/mcpServerDetailsDrawer'

const noop = () => {}

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

export const gallery: ModuleGallery = {
  overlays: [
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
      open: () => ModelDetailsDrawerStore.open(hubModelFixture as any),
    },
    {
      slug: 'overlay-hub-mcp-details-drawer',
      surface: 'modules/hub/modules/mcp/components/McpServerDetailsDrawer',
      title: 'Hub MCP server details (drawer)',
      component: lazyNamed(
        () => import('@/modules/hub/modules/mcp/components/McpServerDetailsDrawer'),
        'McpServerDetailsDrawer',
      ),
      open: () => McpServerDetailsDrawerStore.open(hubMcpFixture as any),
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
  ],
}
