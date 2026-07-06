/**
 * Shard 3 seeded-surface entries (parallel gap grind).
 *
 * OWNED BY SHARD 3 ONLY. Add `SeededSurfaceEntry` objects for your assigned
 * gaps here. Import helpers from './helpers'. Prefix every slug with
 * `seeded-s3-` so slugs never collide across shards. Do NOT edit
 * seededSurfaces.tsx, overlays.tsx, main.tsx, pages.tsx, stories/index.ts,
 * coverage-allowlist.json, or any generated matrix — those are integrator-owned.
 *
 * Scope: modules/llm-provider/**, modules/llm-local-runtime/**, modules/hub/**.
 * See /data/pbya/ziee/tmp/gapgrind-shards.md for your assigned gap list.
 */
import { lazy } from 'react'
import type { SeededSurfaceEntry } from './helpers'
import { holdPatch, lazyNamed, lazyProps } from './helpers'

// A generic non-terminal `now` timestamp for seeded fixtures.
const NOW = new Date().toISOString()

export const shard3Seeded: SeededSurfaceEntry[] = [
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
      const { LlmModelDownload } = await import(
        '@/modules/llm-provider/stores/LlmModelDownload.store'
      )
      const { ViewDownloadDrawer } = await import(
        '@/modules/llm-provider/stores/LlmModelDrawers.store'
      )
      await holdPatch(() => {
        LlmModelDownload.store.setState({
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
      const { LlmModelDownload } = await import(
        '@/modules/llm-provider/stores/LlmModelDownload.store'
      )
      const { ViewDownloadDrawer } = await import(
        '@/modules/llm-provider/stores/LlmModelDrawers.store'
      )
      await holdPatch(() => {
        LlmModelDownload.store.setState({
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
  // ── AvailableVersionsCard: an update check that resolved but has NO published
  //    binaries for this host → the "No published binaries" empty (:162,163). ────
  {
    slug: 'seeded-s3-available-versions-empty',
    title: 'Available runtime versions — none published',
    note: 'updateCheck loaded but readyUpstream.length===0 → "No published binaries" text',
    path: '/',
    initialPath: '/',
    component: lazyProps(
      () => import('@/modules/llm-local-runtime/components/AvailableVersionsCard'),
      'AvailableVersionsCard',
      { engine: 'llamacpp' },
    ),
    setup: async () => {
      const { RuntimeUpdate } = await import(
        '@/modules/llm-local-runtime/stores/RuntimeUpdate.store'
      )
      const { RuntimeConfig } = await import(
        '@/modules/llm-local-runtime/stores/RuntimeConfig.store'
      )
      await holdPatch(() => {
        RuntimeConfig.store.setState({
          gpu: {
            platform: 'linux',
            arch: 'x86_64',
            available: ['cpu'],
            recommended: 'cpu',
          },
          loadingGpu: false,
        } as any)
        RuntimeUpdate.store.setState({
          checking: new Map(),
          updateChecks: new Map([
            [
              'llamacpp',
              {
                engine: 'llamacpp',
                platform: 'linux',
                arch: 'x86_64',
                versions: [],
                latest_version: '',
                has_updates: false,
              },
            ],
          ]),
        } as any)
      })
    },
  },
  // ── AvailableVersionsCard: a ready version WITH a FAILED download snapshot →
  //    the inline progress line (:300) + the failed-error text (:301,302). ───────
  {
    slug: 'seeded-s3-available-versions-failed-row',
    title: 'Available runtime versions — failed download row',
    note: 'a binary_ready version + a failed progress snapshot → the row error line',
    path: '/',
    initialPath: '/',
    component: lazyProps(
      () => import('@/modules/llm-local-runtime/components/AvailableVersionsCard'),
      'AvailableVersionsCard',
      { engine: 'llamacpp' },
    ),
    setup: async () => {
      const { RuntimeUpdate } = await import(
        '@/modules/llm-local-runtime/stores/RuntimeUpdate.store'
      )
      const { RuntimeConfig } = await import(
        '@/modules/llm-local-runtime/stores/RuntimeConfig.store'
      )
      const { RuntimeDownloadProgress } = await import(
        '@/modules/llm-local-runtime/stores/RuntimeDownloadProgress.store'
      )
      await holdPatch(() => {
        RuntimeConfig.store.setState({
          gpu: {
            platform: 'linux',
            arch: 'x86_64',
            available: ['cpu'],
            recommended: 'cpu',
          },
          loadingGpu: false,
        } as any)
        RuntimeUpdate.store.setState({
          checking: new Map(),
          updateChecks: new Map([
            [
              'llamacpp',
              {
                engine: 'llamacpp',
                platform: 'linux',
                arch: 'x86_64',
                latest_version: '1.2.0',
                has_updates: true,
                versions: [
                  {
                    version: '1.2.0',
                    installed: false,
                    installed_backends: [],
                    binary_ready: true,
                    available_backends: ['cpu'],
                    recommended_backend: 'cpu',
                    size_bytes: 734_003_200,
                    prerelease: false,
                  },
                ],
              },
            ],
          ]),
        } as any)
        RuntimeDownloadProgress.store.setState({
          activeByKey: new Map([
            [
              'llamacpp@1.2.0@cpu',
              {
                key: 'llamacpp@1.2.0@cpu',
                engine: 'llamacpp',
                version: '1.2.0',
                backend: 'cpu',
                task_id: 's3-task',
                status: 'failed',
                bytes_received: 0,
                error: 'Download failed: upstream returned 503.',
              },
            ],
          ]),
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
      const { LlmProviderStoreDef } = await import(
        '@/modules/llm-provider/stores/LlmProvider.store'
      )
      await holdPatch(() =>
        LlmProviderStoreDef.store.setState({
          loading: true,
          isInitialized: false,
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
      const { LlmModelDownload } = await import(
        '@/modules/llm-provider/stores/LlmModelDownload.store'
      )
      await holdPatch(() =>
        LlmModelDownload.store.setState({ downloads: [] } as any),
      )
    },
  },
  // ── VersionModelsBlock: an installed engine version with zero models using it →
  //    the "No models use this version" Empty (:80). Pure-props, no store seed. ──
  {
    slug: 'seeded-s3-version-models-empty',
    title: 'Runtime version models — empty',
    note: 'models.length===0 → the "safe to delete" Empty',
    path: '/',
    initialPath: '/',
    component: lazyProps(
      () => import('@/modules/llm-local-runtime/components/VersionModelsBlock'),
      'VersionModelsBlock',
      {
        engine: 'llamacpp',
        versionId: 's3-v1',
        models: [],
        versionOptions: [{ value: 's3-v1', label: '1.0.0' }],
        canManage: true,
        canViewLogs: true,
      },
    ),
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
]
