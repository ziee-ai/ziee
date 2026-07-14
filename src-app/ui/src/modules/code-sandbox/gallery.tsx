/**
 * Dev-gallery seed for the `code-sandbox` module — the resource-limits section
 * (save-error / loading) + the rootfs-versions graceful-degrade state. Auto-
 * discovered by the gallery's runtime registry (`@/dev/gallery/support`); never
 * imported by `module.tsx`, so it is dev-only and tree-shaken from prod.
 */
import type { ModuleGallery } from '@/dev/gallery/support'
import { holdPatch, lazyNamed, whenTrue } from '@/dev/gallery/support'

export const gallery: ModuleGallery = {
  cassette: {
    // The rootfs-versions section of `/settings/sandbox` reads this on mount.
    // A `ready` sandbox with one available release + no installed artifacts →
    // the section renders its available card (Download enabled).
    'CodeSandbox.listRootfsVersions': {
      availability: 'ready',
      host_arch: 'x86_64',
      host_package: 'squashfs',
      conversation_count: 0,
      mcp_server_workspace_count: 0,
      pinned_version: '0.0.6-alpha',
      installed: [],
      draining: [],
      available: [
        {
          version: '0.0.6-alpha',
          published_at: '2026-01-02T00:00:00.000Z',
          draft: false,
          prerelease: true,
          asset_names: [
            'ziee-sandbox-rootfs-x86_64-full.squashfs',
            'ziee-sandbox-rootfs-x86_64-minimal.squashfs',
          ],
        },
      ],
    },
  },
  seeded: [
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
    // ── code_sandbox rootfs versions: sandbox not initialized (disabled). The
    // LIST endpoint returns 200 with the GitHub catalog + availability reason, so
    // the section shows the graceful-degrade warning notice + the available card
    // with Download disabled (never the destructive ErrorState). ────────────────
    {
      slug: 'seeded-sandbox-rootfs-disabled',
      title: 'Code Sandbox rootfs — disabled (graceful degrade)',
      note: 'availability=disabled_in_config + catalog → warning notice + available card, Download disabled',
      path: '/',
      initialPath: '/',
      component: lazyNamed(
        () => import('@/modules/code-sandbox/components/SandboxRootfsVersionsSection'),
        'SandboxRootfsVersionsSection',
      ),
      setup: async () => {
        const { SandboxRootfsVersions } = await import(
          '@/modules/code-sandbox/stores/SandboxRootfsVersions.store'
        )
        await holdPatch(() =>
          SandboxRootfsVersions.store.setState({
            availability: 'disabled_in_config',
            loading: false,
            error: null,
            sseError: null,
            pinnedVersion: null,
            installed: [],
            available: [
              {
                version: '0.0.6-alpha',
                published_at: null,
                draft: false,
                prerelease: true,
                asset_names: [
                  'ziee-sandbox-rootfs-x86_64-full.squashfs',
                  'ziee-sandbox-rootfs-x86_64-minimal.squashfs',
                ],
              },
            ],
            draining: [],
            hostArch: 'x86_64',
            hostPackage: 'squashfs',
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
  ],
}
