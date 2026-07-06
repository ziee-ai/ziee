/**
 * Shard 4 seeded-surface entries (parallel gap grind).
 *
 * OWNED BY SHARD 4 ONLY — MCP + memory + citations + literature.
 * Add `SeededSurfaceEntry` objects for your assigned gaps here. Import helpers
 * from './helpers'. Prefix every slug with `seeded-s4-` so slugs never collide
 * across shards. Do NOT edit seededSurfaces.tsx, overlays.tsx, main.tsx,
 * pages.tsx, stories/index.ts, coverage-allowlist.json, or any generated
 * matrix — those are integrator-owned.
 *
 * See /data/pbya/ziee/tmp/gapgrind-shards.md for your assigned gap list.
 */
import { lazy } from 'react'
import {
  type SeededSurfaceEntry,
  lazyNamed,
  lazyProps,
  holdPatch,
} from './helpers'

/** A project stub — enough for `Stores.ProjectDetail.project` reads (`project.id`). */
const galleryProject = { id: 'proj-s4', name: 'Gallery Project' }

/** Seed the active project so the project-scoped panels mount past their
 *  `if (!project) return null` guard and their effects fetch with a real id. */
const seedProject = async () => {
  const { ProjectDetail } = await import(
    '@/modules/projects/stores/ProjectDetail.store'
  )
  await holdPatch(() =>
    ProjectDetail.store.setState({ project: galleryProject } as any),
  )
}

export const shard4Seeded: SeededSurfaceEntry[] = [
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
      const { ProjectDetail } = await import(
        '@/modules/projects/stores/ProjectDetail.store'
      )
      const { ProjectMcpSettingsStore } = await import(
        '@/modules/mcp/project-extension/stores/ProjectMcpSettings.store'
      )
      await holdPatch(() => {
        ProjectDetail.store.setState({ project: galleryProject } as any)
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
      const { ProjectDetail } = await import(
        '@/modules/projects/stores/ProjectDetail.store'
      )
      const { ProjectMcpSettingsStore } = await import(
        '@/modules/mcp/project-extension/stores/ProjectMcpSettings.store'
      )
      await holdPatch(() => {
        ProjectDetail.store.setState({ project: galleryProject } as any)
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
      const { AppMode } = await import('@/modules/app/AppMode.store')
      const { McpUserPolicy } = await import(
        '@/modules/mcp/stores/McpUserPolicy.store'
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
      const { Form, useForm } = await import('@/components/ui')
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
      const { GroupSystemMcpServersWidget } = await import(
        '@/modules/mcp/widgets/GroupSystemMcpServersWidget.store'
      )
      await holdPatch(() =>
        GroupSystemMcpServersWidget.store.setState({
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
  // ── ProjectBibliographyManagePanel: the Citations.list cassette returns
  //    `{ entries: [] }`, so mounting with an active project drives the initial
  //    fetch (loading <Spin/> arm executes) then resolves to zero entries
  //    (the <Empty/> arm). ─────────────────────────────────────────────────────
  {
    slug: 'seeded-s4-project-bib-manage-empty',
    title: 'Project bibliography manage — empty',
    note: 'initial fetch → loading spinner, then entries.length===0 → <Empty/>',
    path: '/',
    initialPath: '/',
    component: lazyNamed(
      () =>
        import(
          '@/modules/citations/project-extension/components/ProjectBibliographyManagePanel'
        ),
      'ProjectBibliographyManagePanel',
    ),
    setup: seedProject,
  },
  // ── ProjectBibliographyInlinePreview: the Citations.list cassette resolves to
  //    zero entries → count===0 → the "No references yet" manage link. ─────────
  {
    slug: 'seeded-s4-project-bib-inline-empty',
    title: 'Project bibliography inline — empty',
    note: 'count===0 → the "No references yet — click Manage" link',
    path: '/',
    initialPath: '/',
    component: lazyNamed(
      () =>
        import(
          '@/modules/citations/project-extension/components/ProjectBibliographyInlinePreview'
        ),
      'ProjectBibliographyInlinePreview',
    ),
    setup: seedProject,
  },
  // ── LiteratureToolResultCard: a literature_search result whose records array
  //    is empty → the "No records returned" empty text. Pure prop-driven
  //    content renderer (no store). ───────────────────────────────────────────
  {
    slug: 'seeded-s4-lit-tool-result-empty',
    title: 'Literature tool result — empty',
    note: 'sc.records.length===0 → the "No records returned" empty text',
    path: '/',
    initialPath: '/',
    component: lazyProps(
      () => import('@/modules/literature/components/LiteratureToolResultCard'),
      'LiteratureToolResultCard',
      {
        isUser: false,
        content: {
          content_type: 'tool_result',
          content: {
            name: 'literature_search',
            tool_use_id: 'lit-s4',
            structured_content: {
              query: 'crispr base editing safety',
              records: [],
              identified: { europepmc: 0, crossref: 0 },
              after_dedup: 0,
              degraded_sources: [],
              completeness: null,
            },
          },
        },
      },
    ),
  },
]
