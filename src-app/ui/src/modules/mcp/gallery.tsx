/**
 * Dev-gallery seed for the `mcp` module — overlay open-states (group→servers
 * assignment / server drawer / config modal) and seeded surfaces (tool-calls
 * error/loaded, project MCP defaults, user policy, key/value secret editor,
 * group-widget error). Auto-discovered by the gallery's runtime registry
 * (`@/dev/gallery/support`); never imported by `module.tsx`, so it is dev-only
 * and tree-shaken from prod.
 */
import { lazy } from 'react'
import type { ModuleGallery } from '@/dev/gallery/support'
import { holdPatch, lazyNamed, lazyProps } from '@/dev/gallery/support'
import { Stores } from '@ziee/framework/stores'
import { llmGroupsList } from '@/dev/gallery/fixtures/llm-providers'

const group = llmGroupsList.groups[0]

/** A project stub — enough for `Stores.ProjectDetail.project` reads (`project.id`). */
const galleryProject = { id: 'proj-s4', name: 'Gallery Project' }

export const gallery: ModuleGallery = {
  overlays: [
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
  ],
  seeded: [
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
        const { useMcpToolCallsStore } = await import(
          '@/modules/mcp/stores/mcpToolCalls'
        )
        await holdPatch(() =>
          useMcpToolCallsStore.setState({
            error: 'Failed to load tool calls.',
            calls: [],
            loading: false,
          } as any),
        )
      },
    },
    // ── McpToolCallsTab: LOADED with tool-call rows (kit-Table sort/filter). The
    //    grid refetches on mount, so holdPatch re-asserts the seeded rows against
    //    the (empty) cassette. Drives the F1 data-grid sort/filter e2e. ──────────
    {
      slug: 'seeded-mcp-tool-calls-loaded',
      title: 'MCP tool calls — loaded',
      note: 'Stores.McpToolCalls.calls → sortable/filterable grid rows',
      path: '/',
      initialPath: '/',
      component: lazyProps(
        () => import('@/modules/mcp/components/common/McpToolCallsTab'),
        'McpToolCallsTab',
        { serverId: 'srv-1' },
      ),
      setup: async () => {
        const { useMcpToolCallsStore } = await import(
          '@/modules/mcp/stores/mcpToolCalls'
        )
        const mk = (
          id: string,
          tool_name: string,
          status: string,
          source: string,
          duration_ms: number,
        ) => ({
          id,
          tool_name,
          status,
          source,
          duration_ms,
          is_built_in: false,
          is_error: status === 'failed',
          created_at: `2026-01-0${id}T10:00:00Z`,
          started_at: `2026-01-0${id}T10:00:00Z`,
          updated_at: `2026-01-0${id}T10:00:00Z`,
          server_name: 'srv-1',
          content_kinds: [] as string[],
          result_bytes: 0,
          arguments_json: { q: tool_name },
          user_id: 'u-1',
        })
        await holdPatch(() =>
          useMcpToolCallsStore.setState({
            calls: [
              mk('1', 'search', 'completed', 'chat', 120),
              mk('2', 'fetch', 'failed', 'approval', 40),
              mk('3', 'remember', 'completed', 'chat', 8),
            ],
            total: 3,
            currentPage: 1,
            pageSize: 20,
            loading: false,
            hideBuiltIn: false,
            error: null,
          } as never),
        )
      },
    },
    // ── ProjectMcpSettingsPanel: settings still loading (loading && !settings)
    //    → the <Skeleton/> arm. Needs the active project set + the MCP-settings
    //    store held in a loading/no-settings state. ─────────────────────────────
    {
      slug: 'seeded-s4-project-mcp-loading',
      title: 'Project MCP defaults — loading',
      note: 'loading && !settings → the settings <Skeleton/>',
      path: '/',
      initialPath: '/',
      component: lazyNamed(
        () =>
          import(
            '@/modules/mcp/project-extension/components/ProjectMcpSettingsPanel'
          ),
        'ProjectMcpSettingsPanel',
      ),
      setup: async () => {
        const { ProjectDetailDef } = await import(
          '@/modules/projects/stores/projectDetail'
        )
        const { ProjectMcpSettingsStore } = await import(
          '@/modules/mcp/project-extension/stores/ProjectMcpSettings.store'
        )
        await holdPatch(() => {
          ProjectDetailDef.store.setState({ project: galleryProject } as any)
          ProjectMcpSettingsStore.store.setState({
            loading: true,
            settings: null,
          } as any)
        })
      },
    },
    // ── ProjectMcpSettingsPanel: settings loaded but no per-server rules
    //    (autoApproved empty && disabled empty → `noRules`) → the <Empty/> arm. ──
    {
      slug: 'seeded-s4-project-mcp-empty',
      title: 'Project MCP defaults — no rules',
      note: 'noRules (auto_approved + disabled both empty) → the <Empty/> state',
      path: '/',
      initialPath: '/',
      component: lazyNamed(
        () =>
          import(
            '@/modules/mcp/project-extension/components/ProjectMcpSettingsPanel'
          ),
        'ProjectMcpSettingsPanel',
      ),
      setup: async () => {
        const { ProjectDetailDef } = await import(
          '@/modules/projects/stores/projectDetail'
        )
        const { ProjectMcpSettingsStore } = await import(
          '@/modules/mcp/project-extension/stores/ProjectMcpSettings.store'
        )
        await holdPatch(() => {
          ProjectDetailDef.store.setState({ project: galleryProject } as any)
          ProjectMcpSettingsStore.store.setState({
            loading: false,
            settings: {
              approval_mode: 'manual_approve',
              auto_approved_tools: [],
              disabled_servers: [],
            },
          } as any)
        })
      },
    },
    // ── McpUserPolicyCard: no transports allowed (!http && !stdio → noTransports)
    //    → the "Users cannot add any MCP server" warning <Alert/>. multiUserMode
    //    defaults true; seed the policy with an empty allowed_transports so the
    //    form resets to both-off. ────────────────────────────────────────────────
    {
      slug: 'seeded-s4-mcp-user-policy-no-transports',
      title: 'MCP user policy — no transports',
      note: 'noTransports (http + stdio both off) → the no-transports warning alert',
      path: '/',
      initialPath: '/',
      component: lazyNamed(
        () => import('@/modules/mcp/components/system/McpUserPolicyCard'),
        'McpUserPolicyCard',
      ),
      setup: async () => {
        const { AppMode } = await import('@/modules/app/appMode')
        const { McpUserPolicyDef: McpUserPolicy } = await import(
          './stores/mcpUserPolicy'
        )
        await holdPatch(() => {
          AppMode.store.setState({ multiUserMode: true } as any)
          McpUserPolicy.store.setState({
            policy: {
              allowed_transports: [],
              user_stdio_sandbox_flavor: null,
              tool_call_retention_days: 90,
            },
            isInitialized: true,
          } as any)
        })
      },
    },
    // ── KeyValueSecretEditor: an empty field list → the "No <label>s configured."
    //    empty text. A pure prop-driven editor with no store; wrap it in a real
    //    Form whose list field starts empty. ────────────────────────────────────
    {
      slug: 'seeded-s4-kv-secret-empty',
      title: 'MCP key/value secret editor — empty',
      note: 'fields.length===0 → the "No headers configured." empty text',
      path: '/',
      initialPath: '/',
      component: lazy(async () => {
        const { Form, useForm } = await import('@ziee/kit')
        const { KeyValueSecretEditor } = await import(
          '@/modules/mcp/components/common/KeyValueSecretEditor'
        )
        return {
          default: () => {
            const form = useForm<{ headers_entries: never[] }>({
              defaultValues: { headers_entries: [] },
            })
            return (
              <div className="p-4">
                <Form
                  form={form}
                  onSubmit={() => undefined}
                  data-testid="s4-kv-secret-form"
                >
                  <KeyValueSecretEditor
                    name="headers_entries"
                    defaultIsSecret={false}
                    keyPlaceholder="Header-Name"
                    valuePlaceholder="value"
                    labelSingular="header"
                  />
                </Form>
              </div>
            )
          },
        }
      }),
    },
    // ── GroupSystemMcpServersWidget: a load error for this group → the danger
    //    <Text/> arm. Seed the per-group map entry with an error string. ─────────
    {
      slug: 'seeded-s4-group-mcp-widget-error',
      title: 'Group system-MCP widget — error',
      note: 'groupServers[group].error → the danger text',
      path: '/',
      initialPath: '/',
      component: lazyProps(
        () => import('@/modules/mcp/widgets/GroupSystemMcpServersWidget'),
        'GroupSystemMcpServersWidget',
        { group: { id: 'grp-s4', name: 'Gallery Group' } },
      ),
      setup: async () => {
        const { GroupSystemMcpServersWidgetDef } = await import(
          '@/modules/mcp/widgets/groupSystemMcpServersWidget'
        )
        await holdPatch(() =>
          GroupSystemMcpServersWidgetDef.store.setState({
            groupServers: new Map([
              [
                'grp-s4',
                { servers: [], loading: false, error: 'Failed to load servers.' },
              ],
            ]),
          } as any),
        )
      },
    },
  ],
}
