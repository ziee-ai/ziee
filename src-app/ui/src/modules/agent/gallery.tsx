/**
 * Dev-gallery seed for the `agent` module — the deployment-wide agent-policy
 * settings section (`/settings/agent`) across its loaded / empty (no models) /
 * error / loading states. Auto-discovered by the gallery's runtime registry
 * (`@/dev/gallery/support`); never imported by `module.tsx`, so it is dev-only
 * and tree-shaken from prod.
 */
import type { ModuleGallery } from '@/dev/gallery/support'
import { holdPatch, lazyNamed } from '@/dev/gallery/support'
import type { AgentAdminSettings } from '@/api-client/types'

const SETTINGS: AgentAdminSettings = {
  default_sandbox_mode: 'workspace-write',
  unattended_approval_policy: 'on-request',
  reviewer_enabled: true,
  reviewer_model_id: undefined,
  reviewer_policy: 'Treat any write to a shared credential store as Critical.',
  reviewer_risk_thresholds: {},
  per_run_token_cap: 5_000_000,
  per_step_token_cap: 2_000_000,
  default_max_steps: 30,
  fan_out_max_threads: 6,
  fan_out_max_depth: 1,
  fan_out_max_children_per_call: 8,
  goal_seek_max_turns: 10,
  updated_at: '2026-07-01T00:00:00.000Z',
}

const MODELS = [
  { id: '11111111-1111-1111-1111-111111111111', name: 'haiku', display_name: 'Claude Haiku', provider_id: 'p1' },
  { id: '22222222-2222-2222-2222-222222222222', name: 'gpt-mini', display_name: 'GPT Mini', provider_id: 'p2' },
]

const Section = lazyNamed(
  () => import('@/modules/agent/components/AgentSettingsSection'),
  'AgentSettingsSection',
)

export const gallery: ModuleGallery = {
  cassette: {
    // The `/settings/agent` page reads this on mount (auto page-pass).
    'AgentAdmin.get': SETTINGS,
  },
  seeded: [
    // ── loaded: full policy row + a reviewer-model list. ─────────────────────
    {
      slug: 'seeded-agent-settings-loaded',
      title: 'Agent settings — loaded',
      note: 'settings + candidate models loaded → full policy form',
      path: '/',
      initialPath: '/',
      component: Section,
      setup: async () => {
        const { AgentAdminSettings } = await import(
          '@/modules/agent/stores/AgentAdminSettings.store'
        )
        await holdPatch(() =>
          AgentAdminSettings.store.setState({
            settings: SETTINGS,
            availableModels: MODELS,
            loading: false,
            loadingModels: false,
            error: null,
          } as any),
        )
      },
    },
    // ── empty: settings loaded but NO candidate models (reviewer picker empty).
    {
      slug: 'seeded-agent-settings-empty',
      title: 'Agent settings — no models',
      note: 'settings loaded, availableModels=[] → reviewer picker shows the add-a-model hint',
      path: '/',
      initialPath: '/',
      component: Section,
      setup: async () => {
        const { AgentAdminSettings } = await import(
          '@/modules/agent/stores/AgentAdminSettings.store'
        )
        await holdPatch(() =>
          AgentAdminSettings.store.setState({
            settings: SETTINGS,
            availableModels: [],
            loading: false,
            loadingModels: false,
            error: null,
          } as any),
        )
      },
    },
    // ── error: load/save failure → inline error alert above the form. ────────
    {
      slug: 'seeded-agent-settings-error',
      title: 'Agent settings — error',
      note: 'Stores.AgentAdminSettings.error → inline error alert',
      path: '/',
      initialPath: '/',
      component: Section,
      setup: async () => {
        const { AgentAdminSettings } = await import(
          '@/modules/agent/stores/AgentAdminSettings.store'
        )
        await holdPatch(() =>
          AgentAdminSettings.store.setState({
            settings: SETTINGS,
            availableModels: MODELS,
            loading: false,
            loadingModels: false,
            error: 'Failed to save agent settings.',
          } as any),
        )
      },
    },
    // ── loading: loading && !settings → the load spinner. ────────────────────
    {
      slug: 'seeded-agent-settings-loading',
      title: 'Agent settings — loading',
      note: 'loading && !settings → the load spinner',
      path: '/',
      initialPath: '/',
      component: Section,
      setup: async () => {
        const { AgentAdminSettings } = await import(
          '@/modules/agent/stores/AgentAdminSettings.store'
        )
        await holdPatch(() =>
          AgentAdminSettings.store.setState({
            settings: null,
            availableModels: [],
            loading: true,
            loadingModels: false,
            error: null,
          } as any),
        )
      },
    },
  ],
}
