/**
 * Dev-gallery seed for the `llm-local-runtime` module — the runtime engine
 * download drawer + seeded available-versions / live-logs / version-models
 * states. Auto-discovered by the gallery's runtime registry
 * (`@/dev/gallery/support`); never imported by `module.tsx`, so it is dev-only
 * and tree-shaken from prod.
 */
import type { ModuleGallery } from '@/dev/gallery/support'
import { holdPatch, lazyNamed, lazyProps } from '@/dev/gallery/support'
import { Stores } from '@ziee/framework/stores'

export const gallery: ModuleGallery = {
  overlays: [
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
  ],
  seeded: [
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
        const { RuntimeConfigRaw } = await import(
          '@/modules/llm-local-runtime/stores/runtimeConfig'
        )
        await holdPatch(() => {
          RuntimeConfigRaw.setState({
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
        const { RuntimeConfigRaw } = await import(
          '@/modules/llm-local-runtime/stores/runtimeConfig'
        )
        const { RuntimeDownloadProgress } = await import(
          '@/modules/llm-local-runtime/stores/RuntimeDownloadProgress.store'
        )
        await holdPatch(() => {
          RuntimeConfigRaw.setState({
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
  ],
}
