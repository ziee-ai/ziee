/**
 * Dev-gallery seed for the `llm-provider` module — provider/model drawers,
 * download/group widgets, and seeded provider-page states. Auto-discovered by
 * the gallery's runtime registry (`@/dev/gallery/support`); never imported by
 * `module.tsx`, so it is dev-only and tree-shaken from prod.
 */
import { lazy } from 'react'
import type { ModuleGallery } from '@/dev/gallery/support'
import {
  holdForever,
  holdPatch,
  lazyNamed,
  lazyProps,
  whenTrue,
} from '@/dev/gallery/support'
import { Stores } from '@ziee/framework/stores'
import {
  firstEnabledRemoteProviderId,
  llmGroupsList,
  llmProvidersCassette,
  llmProvidersList,
} from '@/dev/gallery/fixtures/llm-providers'

const provider = llmProvidersList.providers[0]
const group = llmGroupsList.groups[0]

const RENAME_PROVIDER = llmProvidersList.providers[0]

// A generic non-terminal `now` timestamp for seeded fixtures.
const NOW = new Date().toISOString()

export const gallery: ModuleGallery = {
  cassette: llmProvidersCassette,
  overlays: [
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
  ],
  seeded: [
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
        const { useLlmProviderStore } = await import(
          '@/modules/llm-provider/stores/llmProvider'
        )
        // Keep the provider seeded so ProviderHeader's find(id) resolves through the
        // recipe's click (holdForever: the lazy chunk may mount after a fixed hold).
        holdForever(() =>
          useLlmProviderStore.setState({
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
        const { LlmModelDownloadStore } = await import(
          '@/modules/llm-provider/stores/llmModelDownload'
        )
        await holdPatch(() =>
          LlmModelDownloadStore.setState({ downloads: [] } as any),
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
        const { useLlmProviderStore } = await import(
          '@/modules/llm-provider/stores/llmProvider'
        )
        const pid =
          firstEnabledRemoteProviderId ?? llmProvidersList.providers[0]?.id ?? 'p1'
        await whenTrue(
          () => useLlmProviderStore.getState().providers.length > 0,
        )
        await holdPatch(() =>
          useLlmProviderStore.setState({
            llmModelsLoading: { [pid]: true },
          } as any),
        )
      },
    },
    // ── AddLocalLlmModelDownloadDrawer (view mode): a FAILED download → the
    //    Download-Progress card (:415) + the failed error-message text (:416,417). ─
    {
      slug: 'seeded-s3-download-view-failed',
      title: 'Download drawer — view failed download',
      note: 'ViewDownloadDrawer open + a failed download → the error-message text',
      path: '/',
      initialPath: '/',
      component: lazyNamed(
        () =>
          import(
            '@/modules/llm-provider/components/llm-models/AddLocalLlmModelDownloadDrawer'
          ),
        'AddLocalLlmModelDownloadDrawer',
      ),
      setup: async () => {
        const { LlmModelDownloadStore } = await import(
          '@/modules/llm-provider/stores/llmModelDownload'
        )
        const { ViewDownloadDrawer } = await import(
          '@/modules/llm-provider/stores/llmModelDrawers'
        )
        await holdPatch(() => {
          LlmModelDownloadStore.setState({
            downloads: [
              {
                id: 's3-dl-failed',
                provider_id: 's3-prov',
                repository_id: 's3-repo',
                status: 'failed',
                error_message: 'Download failed: checksum mismatch on shard 3/7.',
                created_at: NOW,
                started_at: NOW,
                updated_at: NOW,
                request_data: {
                  model_name: 's3-model',
                  display_name: 'Shard-3 Model',
                  repository_path: 'org/shard3-model',
                  main_filename: 'model.safetensors',
                },
              },
            ],
          } as any)
          ViewDownloadDrawer.store.setState({
            open: true,
            downloadId: 's3-dl-failed',
          } as any)
        })
      },
    },
    // ── AddLocalLlmModelDownloadDrawer (view mode): a DOWNLOADING download →
    //    the Cancel-Download footer button (:368,369) + the progress card (:415). ─
    {
      slug: 'seeded-s3-download-view-downloading',
      title: 'Download drawer — view in-flight download',
      note: 'ViewDownloadDrawer open + a downloading download → cancel button + progress bar',
      path: '/',
      initialPath: '/',
      component: lazyNamed(
        () =>
          import(
            '@/modules/llm-provider/components/llm-models/AddLocalLlmModelDownloadDrawer'
          ),
        'AddLocalLlmModelDownloadDrawer',
      ),
      setup: async () => {
        const { LlmModelDownloadStore } = await import(
          '@/modules/llm-provider/stores/llmModelDownload'
        )
        const { ViewDownloadDrawer } = await import(
          '@/modules/llm-provider/stores/llmModelDrawers'
        )
        await holdPatch(() => {
          LlmModelDownloadStore.setState({
            downloads: [
              {
                id: 's3-dl-active',
                provider_id: 's3-prov',
                repository_id: 's3-repo',
                status: 'downloading',
                created_at: NOW,
                started_at: NOW,
                updated_at: NOW,
                progress_data: {
                  current: 512,
                  total: 1024,
                  phase: 'downloading',
                  message: 'Fetching weights…',
                  speed_bps: 5_242_880,
                  eta_seconds: 120,
                },
                request_data: {
                  model_name: 's3-model',
                  display_name: 'Shard-3 Model',
                  repository_path: 'org/shard3-model',
                  main_filename: 'model.safetensors',
                },
              },
            ],
          } as any)
          ViewDownloadDrawer.store.setState({
            open: true,
            downloadId: 's3-dl-active',
          } as any)
        })
      },
    },
    // ── LocalProviderSettings: providers still loading + the URL's providerId not
    //    yet in the list → the full-page <Loading/> guard (:35). ───────────────────
    {
      slug: 'seeded-s3-local-provider-loading',
      title: 'Local provider settings — loading',
      note: '!currentProvider && (loading || !isInitialized) → the page <Loading/>',
      path: '/gallery/:providerId',
      initialPath: '/gallery/s3-loading-prov',
      component: lazyNamed(
        () => import('@/modules/llm-provider/components/LocalProviderSettings'),
        'LocalProviderSettings',
      ),
      setup: async () => {
        const { useLlmProviderStore } = await import(
          '@/modules/llm-provider/stores/llmProvider'
        )
        // holdForever (not holdPatch): this lazy component can mount after a fixed
        // hold window ends under the full pass, so assert on a permanent interval.
        holdForever(() =>
          useLlmProviderStore.setState({
            loading: true,
            isInitialized: false,
            providers: [],
          } as any),
        )
      },
    },
    // ── DownloadsSection: no downloads match this provider → the `return null`
    //    empty arm (:21). Renders nothing (no testid) — counts in coverage. ────────
    {
      slug: 'seeded-s3-downloads-section-empty',
      title: 'Downloads section — empty',
      note: 'providerDownloads.length===0 → renders nothing',
      path: '/',
      initialPath: '/',
      component: lazyProps(
        () => import('@/modules/llm-provider/components/downloads/DownloadsSection'),
        'DownloadsSection',
        { providerId: 's3-empty-prov' },
      ),
      setup: async () => {
        const { LlmModelDownloadStore } = await import(
          '@/modules/llm-provider/stores/llmModelDownload'
        )
        await holdPatch(() =>
          LlmModelDownloadStore.setState({ downloads: [] } as any),
        )
      },
    },
    // ── LLMProviderGroupWidget: the per-instance local store's providers GET
    //    fails → the inline danger error text (:57). The widget owns a
    //    `defineLocalStore` (no global setState handle), so we force the error by
    //    500-ing ONLY the group-providers endpoint via a narrow fetch shim before
    //    the widget mounts + fetches. ─────────────────────────────────────────────
    {
      slug: 'seeded-s3-group-widget-error',
      title: 'LLM providers group widget — load error',
      note: 'group-providers GET 500 → the widget-store error branch',
      path: '/',
      initialPath: '/',
      component: lazy(async () => {
        const { LLMProviderGroupWidget } = await import(
          '@/modules/llm-provider/widgets/LLMProviderGroupWidget'
        )
        const group = {
          id: 's3-grp-err',
          name: 'Gallery Group',
          description: '',
          created_at: NOW,
          updated_at: NOW,
          is_active: true,
          is_default: false,
          is_system: false,
          permissions: [],
        }
        return {
          default: () => {
            // One-time narrow shim: 500 the group-providers read so the local
            // widget store lands on its catch/error branch. Everything else falls
            // through to the gallery mock. Installed in render (before child
            // effects fire the fetch); the disposable seeded page never restores.
            const w = window as unknown as { __s3GroupFetchPatched?: boolean }
            if (!w.__s3GroupFetchPatched) {
              w.__s3GroupFetchPatched = true
              const orig = window.fetch.bind(window)
              window.fetch = (input: any, init?: any) => {
                const url =
                  typeof input === 'string'
                    ? input
                    : input instanceof URL
                      ? input.href
                      : input.url
                const method = (
                  init?.method ??
                  (input instanceof Request ? input.method : 'GET')
                ).toUpperCase()
                if (
                  method === 'GET' &&
                  /\/api\/groups\/[^/]+\/providers$/.test(url)
                ) {
                  return Promise.resolve(
                    new Response(
                      JSON.stringify({ error: 'Gallery forced error' }),
                      { status: 500, headers: { 'Content-Type': 'application/json' } },
                    ),
                  )
                }
                return orig(input, init)
              }
            }
            return <LLMProviderGroupWidget group={group as any} />
          },
        }
      }),
    },
  ],
}
