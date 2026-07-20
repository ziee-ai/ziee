import { ApiClient } from '@/api-client'
import { type DrainEntry, type InstallTaskState, type RootfsArtifact, type RootfsRelease, type SandboxAvailability, type SSEInstallCompleteData, type SSEInstallConnectedData, type SSEInstallFailedData, type SSEInstallProgressData, type SwapOutcome, type VersionStatus } from '@/api-client/types'
import { Permissions } from '@/api-client/permissions'
import { hasPermissionNow } from '@/core/permissions'
import { defineStore } from '@ziee/framework/store-kit'
import { reconcileInitialTask } from './installTaskReconcile'

/** Per-(version, arch, flavor, package) action state — drives the install /
 *  set-pin / delete buttons' loading flags. */
interface ActionState {
  installing?: boolean
  pinning?: boolean
  deleting?: boolean
}

function rowKey(version: string, arch: string, flavor: string, pkg: string): string {
  return `${version}::${arch}::${flavor}::${pkg}`
}

function taskRowKey(t: InstallTaskState): string {
  return rowKey(t.version, t.arch, t.flavor, t.package)
}

function setAction(s: { actions: Record<string, ActionState> }, key: string, patch: ActionState) {
  const cur = s.actions[key] ?? {}
  s.actions[key] = { ...cur, ...patch }
}

function clearAction(s: { actions: Record<string, ActionState> }, key: string) {
  delete s.actions[key]
}

// Module-scoped AbortController for the SSE stream — outside immer state
// (AbortController isn't draftable). Doubles as a double-subscribe guard.
let sseController: AbortController | null = null
// Audit Net3: reconnect state so a server bounce / 503-cap backs off.
let sseReconnectAttempts = 0
let sseReconnectTimer: ReturnType<typeof setTimeout> | null = null
const SSE_MAX_RECONNECT_ATTEMPTS = 5
const SSE_RECONNECT_DELAY_MS = 3000

function cleanupSse() {
  sseController?.abort()
  sseController = null
  if (sseReconnectTimer) {
    clearTimeout(sseReconnectTimer)
    sseReconnectTimer = null
  }
  sseReconnectAttempts = 0
}

export const SandboxRootfsVersions = defineStore('SandboxRootfsVersions', {
  immer: true,
  state: {
    pinnedVersion: null as string | null,
    installed: [] as RootfsArtifact[],
    /** Releases on GitHub (catalog). Empty if GitHub was unreachable. */
    available: [] as RootfsRelease[],
    /** Live mounts the server registered — keyed by artifact_id. */
    draining: [] as DrainEntry[],
    /** Count of per-conversation workspace dirs. */
    conversationCount: 0,
    /** Count of per-MCP-server workspace dirs. */
    mcpServerWorkspaceCount: 0,
    /** Server-authoritative host CPU arch + rootfs package format. */
    hostArch: null as string | null,
    hostPackage: null as string | null,
    /** Whether code_sandbox is initialized, else the machine-readable reason.
     * When not `'ready'` the LIST endpoint still returns 200 with the GitHub
     * catalog (installed/pinned empty) — the section renders a graceful notice
     * instead of a destructive error. Defaults to `'ready'` so the working UI is
     * unchanged until a degraded response arrives. */
    availability: 'ready' as SandboxAvailability,
    /** Outcome of the last set-pin call. */
    lastSwap: null as SwapOutcome | null,
    loading: false,
    /** Data-load failure (the rootfs-status GET). Rendered as a destructive
     * ErrorState. NEVER holds SSE/transport state — that lives in `sseError`. */
    error: null as string | null,
    /** Live-progress SSE transport state (disconnect/reconnect/permanent
     * failure). Kept OUT of `error` so a background reconnect blip never
     * surfaces as a raw destructive "SSE disconnected…" string in user copy. */
    sseError: null as string | null,
    actions: {} as Record<string, ActionState>,
    /** Live install task state, keyed by `<version>::<arch>::<flavor>::<package>`. */
    installTasks: {} as Record<string, InstallTaskState>,
    /** True once the SSE subscription emitted its `connected` event. */
    sseConnected: false,
  },
  actions: set => {
    const loadStatus = async (opts?: { pruneFailed?: boolean }) => {
      set(s => {
        s.loading = true
        s.error = null
      })
      try {
        const res: VersionStatus = await ApiClient.CodeSandbox.listRootfsVersions(undefined)
        set(s => {
          s.pinnedVersion = res.pinned_version ?? null
          s.installed = res.installed
          s.available = res.available
          s.draining = res.draining
          s.conversationCount = res.conversation_count
          s.mcpServerWorkspaceCount = res.mcp_server_workspace_count
          s.hostArch = res.host_arch ?? null
          s.hostPackage = res.host_package ?? null
          s.availability = res.availability
          // Prune any install task whose artifact has landed — keeps installTasks
          // bounded while letting the SSE `complete` handler hold a completed
          // task until this reload arrives (aggregate bar stays monotonic).
          for (const a of res.installed) {
            delete s.installTasks[rowKey(a.version, a.arch, a.flavor, a.package)]
          }
          // Clear terminal-failed tasks ONLY on explicit Refresh/mount — NOT on
          // the SSE `complete` auto-reload, or a sibling flavor's success would
          // erase a still-failed flavor's red bar.
          if (opts?.pruneFailed) {
            for (const key of Object.keys(s.installTasks)) {
              if (s.installTasks[key].status === 'failed') delete s.installTasks[key]
            }
          }
          s.loading = false
        })
      } catch (e: any) {
        set(s => {
          s.error = e?.message ?? 'Failed to load rootfs versions'
          s.loading = false
        })
      }
    }

    const subscribeToInstallProgress = async () => {
      // Audit Net3: hard guard against double-subscribe (open stream OR pending
      // reconnect timer).
      if (sseController || sseReconnectTimer) return
      try {
        await ApiClient.CodeSandbox.subscribeRootfsInstallProgress(undefined, {
          SSE: {
            __init: ({ abortController }: { abortController: AbortController }) => {
              sseController = abortController
              sseReconnectAttempts = 0
            },
            connected: (_d: SSEInstallConnectedData) => {
              set(s => {
                s.sseConnected = true
                s.sseError = null
              })
            },
            taskStarted: (t: InstallTaskState) => {
              const key = taskRowKey(t)
              set(s => {
                s.installTasks[key] = t
                setAction(s, key, { installing: true })
              })
            },
            taskState: (t: InstallTaskState) => {
              const key = taskRowKey(t)
              set(s => {
                s.installTasks[key] = t
                if (t.status === 'running') {
                  setAction(s, key, { installing: true })
                } else if (t.status === 'completed' || t.status === 'failed') {
                  // A reconnect snapshot can deliver an already-terminal task
                  // whose event fired while disconnected — clear the installing
                  // flag or the row stays stuck spinning.
                  clearAction(s, key)
                }
              })
              if (t.status === 'completed') void loadStatus()
            },
            progress: (d: SSEInstallProgressData) => {
              set(s => {
                for (const key of Object.keys(s.installTasks)) {
                  const t = s.installTasks[key]
                  if (t.task_id === d.task_id) {
                    // Don't let a late progress event resurrect a terminal task.
                    if (t.status === 'completed' || t.status === 'failed') continue
                    t.phase = d.phase
                    t.message = d.message
                  }
                }
              })
            },
            complete: (d: SSEInstallCompleteData) => {
              set(s => {
                for (const key of Object.keys(s.installTasks)) {
                  const t = s.installTasks[key]
                  if (t.task_id === d.task_id) {
                    // Keep it 'complete' (→100%) so the aggregate bar stays
                    // monotonic across the loadStatus round-trip; loadStatus
                    // prunes once the artifact lands.
                    t.status = 'completed'
                    t.phase = 'complete'
                    t.artifact_id = d.artifact_id
                    t.bytes_downloaded = d.bytes_downloaded
                    t.duration_ms = d.duration_ms
                    clearAction(s, key)
                  }
                }
              })
              void loadStatus()
            },
            failed: (d: SSEInstallFailedData) => {
              set(s => {
                for (const key of Object.keys(s.installTasks)) {
                  const t = s.installTasks[key]
                  if (t.task_id === d.task_id) {
                    t.status = 'failed'
                    t.phase = 'failed'
                    t.error = d.error
                    clearAction(s, key)
                  }
                }
                // Surface via the per-version progress message only (setting the
                // global s.error too produced a duplicate top-level Alert).
              })
            },
            error: (msg: string) => {
              set(s => {
                s.sseConnected = false
                s.sseError = msg
              })
            },
            default: (_event: string, _data: unknown) => {},
          },
        } as any)
      } catch (e) {
        // Audit Net3: bounded reconnect (mirrors LlmModelDownload).
        sseController = null
        set(s => {
          s.sseConnected = false
        })
        sseReconnectAttempts += 1
        if (sseReconnectAttempts < SSE_MAX_RECONNECT_ATTEMPTS) {
          set(s => {
            s.sseError = `SSE disconnected; reconnecting (attempt ${sseReconnectAttempts}/${SSE_MAX_RECONNECT_ATTEMPTS})`
          })
          sseReconnectTimer = setTimeout(() => {
            sseReconnectTimer = null
            void subscribeToInstallProgress()
          }, SSE_RECONNECT_DELAY_MS)
        } else {
          set(s => {
            s.sseError =
              e instanceof Error
                ? `SSE failed after ${SSE_MAX_RECONNECT_ATTEMPTS} attempts: ${e.message}`
                : `SSE failed after ${SSE_MAX_RECONNECT_ATTEMPTS} attempts`
          })
          sseReconnectAttempts = 0
        }
      }
    }

    return {
      loadStatus,
      subscribeToInstallProgress,
      installVersion: async (version: string, arch: string, flavor: string, pkg: string) => {
        const key = rowKey(version, arch, flavor, pkg)
        set(s => {
          setAction(s, key, { installing: true })
          s.error = null
        })
        try {
          // 202 Accepted — server returns InstallTaskState immediately; live
          // progress streams through the SSE subscription.
          const initial = await ApiClient.CodeSandbox.installRootfsVersion({
            version,
            arch,
            flavor,
            package: pkg,
          })
          set(s => {
            // Race guard: the SSE `taskStarted`/`progress` events (same task_id)
            // may already have created + advanced this task while this POST was
            // in flight. Keep the SSE-tracked task if present so a late reply
            // (phase: null) can't clobber an in-flight download back to "queued".
            s.installTasks[key] = reconcileInitialTask(s.installTasks[key], initial)
          })
        } catch (e: any) {
          set(s => {
            s.error = e?.message ?? `Failed to install ${version}`
            clearAction(s, key)
          })
        }
      },
      setPin: async (version: string): Promise<boolean> => {
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
            // Response carries the full VersionStatus — refresh draining +
            // counts too, or per-row Draining tags stay stale after a swap.
            s.draining = res.status.draining
            s.conversationCount = res.status.conversation_count
            s.mcpServerWorkspaceCount = res.status.mcp_server_workspace_count
            s.hostArch = res.status.host_arch ?? null
            s.hostPackage = res.status.host_package ?? null
            s.availability = res.status.availability
            s.lastSwap = res.swap
          })
          return true
        } catch (e: any) {
          set(s => {
            s.error = e?.message ?? `Failed to set pin to ${version}`
          })
          return false
        } finally {
          set(s => {
            clearAction(s, key)
          })
        }
      },
      deleteArtifact: async (id: string): Promise<boolean> => {
        const key = `del::${id}`
        set(s => {
          setAction(s, key, { deleting: true })
          s.error = null
        })
        try {
          const res: VersionStatus = await ApiClient.CodeSandbox.deleteRootfsVersion({ id })
          set(s => {
            s.pinnedVersion = res.pinned_version ?? null
            s.installed = res.installed
            s.available = res.available
            s.draining = res.draining
            s.conversationCount = res.conversation_count
            s.mcpServerWorkspaceCount = res.mcp_server_workspace_count
            s.hostArch = res.host_arch ?? null
            s.hostPackage = res.host_package ?? null
            s.availability = res.availability
          })
          return true
        } catch (e: any) {
          set(s => {
            s.error = e?.message ?? 'Failed to delete artifact'
          })
          return false
        } finally {
          set(s => {
            clearAction(s, key)
          })
        }
      },
    }
  },
  init: ({ on, actions, onCleanup }) => {
    // Eager-load status + open the install-progress SSE on mount. The backend
    // enforces the permission, so always load (a permission snapshot here runs
    // before auth populates and would permanently skip the load).
    void (async () => {
      await actions.loadStatus({ pruneFailed: true })
      void actions.subscribeToInstallProgress()
    })()
    // Cross-device sync: another admin installed/evicted/deleted a version.
    // Self-gated (sync events fire after auth populates, so the snapshot is reliable).
    const reload = () => {
      if (!hasPermissionNow(Permissions.CodeSandboxEnvironmentsRead)) return
      void actions.loadStatus()
    }
    on('sync:code_sandbox_rootfs_version', reload)
    on('sync:reconnect', reload)
    onCleanup(cleanupSse)
  },
})

export const useSandboxRootfsVersionsStore = SandboxRootfsVersions.store
