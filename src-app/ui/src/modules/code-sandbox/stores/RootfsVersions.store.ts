import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { immer } from 'zustand/middleware/immer'
import { ApiClient } from '@/api-client'
import type {
  RootfsArtifact,
  RootfsRelease,
  SwapOutcome,
  VersionStatus,
} from '@/api-client/types'

/**
 * Per-(version, arch, flavor, package) action state. Drives the
 * install / set-pin / delete buttons' loading flags. Keyed by a
 * synthetic id — for installed rows the artifact_id; for
 * not-yet-downloaded GitHub-side rows the synthetic
 * `<version>::<arch>::<flavor>::<package>`.
 */
interface ActionState {
  installing?: boolean
  pinning?: boolean
  deleting?: boolean
}

interface RootfsVersionsStore {
  pinnedVersion: string | null
  installed: RootfsArtifact[]
  /** Releases on GitHub (catalog). Empty array if GitHub was unreachable. */
  available: RootfsRelease[]
  /** Outcome of the last set-pin call. Drives the "n draining" indicator. */
  lastSwap: SwapOutcome | null
  loading: boolean
  error: string | null
  actions: Record<string, ActionState>

  __init__: {
    rootfsVersions?: () => Promise<void>
  }

  loadStatus: () => Promise<void>
  installVersion: (
    version: string,
    arch: string,
    flavor: string,
    pkg: string,
  ) => Promise<void>
  setPin: (version: string) => Promise<void>
  deleteArtifact: (id: string) => Promise<void>
}

function rowKey(
  version: string,
  arch: string,
  flavor: string,
  pkg: string,
): string {
  return `${version}::${arch}::${flavor}::${pkg}`
}

function setAction(
  s: { actions: Record<string, ActionState> },
  key: string,
  patch: ActionState,
) {
  const cur = s.actions[key] ?? {}
  s.actions[key] = { ...cur, ...patch }
}

function clearAction(
  s: { actions: Record<string, ActionState> },
  key: string,
) {
  delete s.actions[key]
}

export const useRootfsVersionsStore = create<RootfsVersionsStore>()(
  subscribeWithSelector(
    immer((set, get) => ({
      pinnedVersion: null,
      installed: [],
      available: [],
      lastSwap: null,
      loading: false,
      error: null,
      actions: {},

      __init__: {
        rootfsVersions: async () => {
          await get().loadStatus()
        },
      },

      loadStatus: async () => {
        set(s => {
          s.loading = true
          s.error = null
        })
        try {
          const res: VersionStatus = await ApiClient.CodeSandbox.getRootfsVersions(
            undefined,
          )
          set(s => {
            s.pinnedVersion = res.pinned_version ?? null
            s.installed = res.installed
            s.available = res.available
            s.loading = false
          })
        } catch (e: any) {
          set(s => {
            s.error = e?.message ?? 'Failed to load rootfs versions'
            s.loading = false
          })
        }
      },

      installVersion: async (version, arch, flavor, pkg) => {
        const key = rowKey(version, arch, flavor, pkg)
        set(s => {
          setAction(s, key, { installing: true })
          s.error = null
        })
        try {
          await ApiClient.CodeSandbox.installRootfsVersion({
            version,
            arch,
            flavor,
            package: pkg,
          })
          // Refresh status so the new row shows as installed.
          await get().loadStatus()
        } catch (e: any) {
          set(s => {
            s.error = e?.message ?? `Failed to install ${version}`
          })
        } finally {
          set(s => {
            clearAction(s, key)
          })
        }
      },

      setPin: async (version: string) => {
        // Synthetic key — applies to the WHOLE version, not a single
        // (arch, flavor, package) row.
        const key = `pin::${version}`
        set(s => {
          setAction(s, key, { pinning: true })
          s.error = null
        })
        try {
          const res = await ApiClient.CodeSandbox.setRootfsPin({ version })
          set(s => {
            s.pinnedVersion = res.status.pinned_version ?? null
            s.installed = res.status.installed
            s.available = res.status.available
            s.lastSwap = res.swap
          })
        } catch (e: any) {
          set(s => {
            s.error = e?.message ?? `Failed to set pin to ${version}`
          })
        } finally {
          set(s => {
            clearAction(s, key)
          })
        }
      },

      deleteArtifact: async (id: string) => {
        const key = `del::${id}`
        set(s => {
          setAction(s, key, { deleting: true })
          s.error = null
        })
        try {
          const res: VersionStatus =
            await ApiClient.CodeSandbox.deleteRootfsVersion({ id })
          set(s => {
            s.pinnedVersion = res.pinned_version ?? null
            s.installed = res.installed
            s.available = res.available
          })
        } catch (e: any) {
          set(s => {
            s.error = e?.message ?? 'Failed to delete artifact'
          })
        } finally {
          set(s => {
            clearAction(s, key)
          })
        }
      },
    })),
  ),
)
