import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { immer } from 'zustand/middleware/immer'
import { ApiClient } from '@/api-client'
import type {
  DrainEntry,
  InstallTaskState,
  RootfsArtifact,
  RootfsRelease,
  SSEInstallCompleteData,
  SSEInstallConnectedData,
  SSEInstallFailedData,
  SSEInstallProgressData,
  SwapOutcome,
  VersionStatus,
} from '@/api-client/types'
import { Permissions } from '@/api-client/types'
import { hasPermissionNow } from '@/core/permissions'
import { Stores } from '@/core/stores'

/**
 * Per-(version, arch, flavor, package) action state. Drives the
 * install / set-pin / delete buttons' loading flags.
 */
interface ActionState {
  installing?: boolean
  pinning?: boolean
  deleting?: boolean
}

interface SandboxRootfsVersionsStore {
  pinnedVersion: string | null
  installed: RootfsArtifact[]
  /** Releases on GitHub (catalog). Empty array if GitHub was unreachable. */
  available: RootfsRelease[]
  /** Live mounts the server has registered — keyed by artifact_id.
   * Used to render per-row in-flight counts + a per-row "Draining" tag
   * during a pin-change-driven evict cycle. */
  draining: DrainEntry[]
  /** Count of per-conversation workspace dirs. */
  conversationCount: number
  /** Count of per-MCP-server workspace dirs. */
  mcpServerWorkspaceCount: number
  /** Server-authoritative host CPU arch + rootfs package format. Null until the
   *  first status load (then the UI prefers these over guessing from installed
   *  artifacts, so a fresh host offers the artifact its backend can mount). */
  hostArch: string | null
  hostPackage: string | null
  /** Outcome of the last set-pin call. */
  lastSwap: SwapOutcome | null
  loading: boolean
  error: string | null
  actions: Record<string, ActionState>
  /** Live install task state, keyed by `<version>::<arch>::<flavor>::<package>`.
   *  Populated by the SSE subscriber; cleared when the task completes
   *  (a `loadStatus` is fired so the new row appears as Downloaded). */
  installTasks: Record<string, InstallTaskState>
  /** True once the SSE subscription has emitted its `connected` event. */
  sseConnected: boolean

  __init__: {
    // `__store__` fires once on the first access of ANY store property
    // (see core/stores.ts) — i.e. when the section first reads
    // `pinnedVersion`/`installed`/etc. A named key here would only fire
    // if a component read a property of that exact name, which nothing
    // does — so the eager-load never ran (page stuck on "No rootfs
    // versions yet" until a manual Refresh).
    __store__?: () => Promise<void>
  }
  __destroy__?: () => void

  /** `pruneFailed` clears terminal-failed tasks (stuck red bars). Only set it
   *  on an explicit Refresh / mount — NOT on the auto-reload fired by the SSE
   *  `complete` handler, or a sibling flavor's success would erase a still-
   *  failed flavor's error bar. */
  loadStatus: (opts?: { pruneFailed?: boolean }) => Promise<void>
  installVersion: (
    version: string,
    arch: string,
    flavor: string,
    pkg: string,
  ) => Promise<void>
  setPin: (version: string) => Promise<boolean>
  deleteArtifact: (id: string) => Promise<boolean>
  /** Open the SSE channel to receive install progress events. */
  subscribeToInstallProgress: () => Promise<void>
}

function rowKey(
  version: string,
  arch: string,
  flavor: string,
  pkg: string,
): string {
  return `${version}::${arch}::${flavor}::${pkg}`
}

function taskRowKey(t: InstallTaskState): string {
  return rowKey(t.version, t.arch, t.flavor, t.package)
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

// Module-scoped AbortController for the SSE stream — sits outside
// immer state because AbortController isn't draftable. Doubles as a
// guard against double-subscribing.
let sseController: AbortController | null = null
// Audit Net3: track reconnect state so a server bounce or 503-cap
// rejection backs off instead of hot-looping the subscribe call.
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

export const useSandboxRootfsVersionsStore = create<SandboxRootfsVersionsStore>()(
  subscribeWithSelector(
    immer((set, get) => ({
      pinnedVersion: null,
      installed: [],
      available: [],
      draining: [],
      conversationCount: 0,
      mcpServerWorkspaceCount: 0,
      hostArch: null,
      hostPackage: null,
      lastSwap: null,
      loading: false,
      error: null,
      actions: {},
      installTasks: {},
      sseConnected: false,

      __init__: {
        __store__: async () => {
          // Eager-load the status + open the install-progress SSE on
          // mount (mirrors the sibling SandboxResourceLimits store).
          //
          // NOTE: a previous attempt gated this on a `hasPermissionNow`
          // snapshot to spare a resource-limits-only admin a couple of
          // backend-403s. That snapshot runs the instant the component
          // first reads the store — frequently BEFORE auth populates —
          // so it returned false and PERMANENTLY skipped the load,
          // leaving the page empty for legit admins once `usePermission`
          // reactively caught up. The backend already enforces the
          // permission, so always loading is correct; the 403s for a
          // resource-limits-only admin are harmless + backend-rejected.
          await get().loadStatus({ pruneFailed: true })
          void get().subscribeToInstallProgress()

          // Cross-device sync: another admin installed/evicted/deleted a
          // rootfs version. Refetch the list. Self-gated (unlike the
          // mount-time load above, sync events fire after auth is
          // populated, so the permission snapshot is reliable here).
          const reload = () => {
            if (!hasPermissionNow(Permissions.CodeSandboxEnvironmentsRead))
              return
            void get().loadStatus()
          }
          Stores.EventBus.on(
            'sync:code_sandbox_rootfs_version',
            reload,
            'SandboxRootfsVersions',
          )
          Stores.EventBus.on(
            'sync:reconnect',
            reload,
            'SandboxRootfsVersions',
          )
        },
      },

      __destroy__: () => {
        cleanupSse()
      },

      loadStatus: async (opts?: { pruneFailed?: boolean }) => {
        set(s => {
          s.loading = true
          s.error = null
        })
        try {
          const res: VersionStatus = await ApiClient.CodeSandbox.listRootfsVersions(
            undefined,
          )
          set(s => {
            s.pinnedVersion = res.pinned_version ?? null
            s.installed = res.installed
            s.available = res.available
            s.draining = res.draining
            s.conversationCount = res.conversation_count
            s.mcpServerWorkspaceCount = res.mcp_server_workspace_count
            s.hostArch = res.host_arch ?? null
            s.hostPackage = res.host_package ?? null
            // Prune any install task whose artifact has now landed — keeps
            // installTasks bounded while letting the SSE `complete` handler hold
            // a completed task (phase='complete' → 100%) until this reload
            // arrives, so the per-version aggregate bar stays monotonic.
            for (const a of res.installed) {
              delete s.installTasks[rowKey(a.version, a.arch, a.flavor, a.package)]
            }
            // Clear terminal-failed tasks ONLY on an explicit Refresh/mount
            // (pruneFailed) — NOT on the auto-reload the SSE `complete` handler
            // fires, or a sibling flavor's success would silently erase a
            // still-failed flavor's red 'exception' bar. A failed task has no
            // artifact, so the landed-artifact prune above never reaches it.
            if (opts?.pruneFailed) {
              for (const key of Object.keys(s.installTasks)) {
                if (s.installTasks[key].status === 'failed') {
                  delete s.installTasks[key]
                }
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
      },

      installVersion: async (version, arch, flavor, pkg) => {
        const key = rowKey(version, arch, flavor, pkg)
        set(s => {
          setAction(s, key, { installing: true })
          s.error = null
        })
        try {
          // 202 Accepted — server returns InstallTaskState immediately;
          // live progress streams through the SSE subscription.
          const initial = await ApiClient.CodeSandbox.installRootfsVersion({
            version,
            arch,
            flavor,
            package: pkg,
          })
          set(s => {
            s.installTasks[key] = initial
          })
        } catch (e: any) {
          set(s => {
            s.error = e?.message ?? `Failed to install ${version}`
            clearAction(s, key)
          })
        }
      },

      setPin: async (version: string) => {
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
            // Mirror loadStatus/deleteArtifact: the response carries the full
            // VersionStatus, so refresh draining + workspace counts too,
            // otherwise per-row Draining tags stay stale right after a swap.
            s.draining = res.status.draining
            s.conversationCount = res.status.conversation_count
            s.mcpServerWorkspaceCount = res.status.mcp_server_workspace_count
            s.hostArch = res.status.host_arch ?? null
            s.hostPackage = res.status.host_package ?? null
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
            s.draining = res.draining
            s.conversationCount = res.conversation_count
            s.mcpServerWorkspaceCount = res.mcp_server_workspace_count
            s.hostArch = res.host_arch ?? null
            s.hostPackage = res.host_package ?? null
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

      subscribeToInstallProgress: async () => {
        // Audit Net3: hard guard against double-subscribe — both an
        // already-open stream AND a pending reconnect timer would
        // result in two SSE sessions racing if a second caller fires
        // mid-backoff.
        if (sseController || sseReconnectTimer) return
        try {
          await ApiClient.CodeSandbox.subscribeRootfsInstallProgress(
            undefined,
            {
              SSE: {
                __init: ({ abortController }: { abortController: AbortController }) => {
                  sseController = abortController
                  sseReconnectAttempts = 0
                },
                connected: (_d: SSEInstallConnectedData) => {
                  set(s => {
                    s.sseConnected = true
                    // Clear any stale "disconnected; reconnecting" message now
                    // that the stream is live again.
                    s.error = null
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
                    } else if (
                      t.status === 'completed' ||
                      t.status === 'failed'
                    ) {
                      // A reconnect snapshot can deliver an ALREADY-terminal task
                      // whose complete/failed event fired while we were
                      // disconnected — those won't re-fire, so clear the
                      // installing flag here or the row stays stuck spinning.
                      clearAction(s, key)
                    }
                  })
                  // A terminal-completed task seen via the snapshot also means
                  // the artifact list is stale; refresh so the row flips to
                  // "Downloaded" (mirrors the `complete` handler).
                  if (t.status === 'completed') {
                    void get().loadStatus()
                  }
                },
                progress: (d: SSEInstallProgressData) => {
                  set(s => {
                    // Walk active tasks to find the matching task_id.
                    for (const key of Object.keys(s.installTasks)) {
                      const t = s.installTasks[key]
                      if (t.task_id === d.task_id) {
                        // Don't let a late/out-of-order progress event resurrect
                        // a task that already reached a terminal status.
                        if (t.status === 'completed' || t.status === 'failed') {
                          continue
                        }
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
                        // Keep the task marked 'complete' (phasePercent → 100)
                        // so a version's aggregate bar stays monotonic across
                        // the loadStatus round-trip below; loadStatus prunes the
                        // task once the artifact lands (bounding growth). Don't
                        // delete here — that would briefly drop the flavor's
                        // contribution to 5% until the reload arrives.
                        t.status = 'completed'
                        t.phase = 'complete'
                        t.artifact_id = d.artifact_id
                        t.bytes_downloaded = d.bytes_downloaded
                        t.duration_ms = d.duration_ms
                        clearAction(s, key)
                      }
                    }
                  })
                  // Reload the artifact list so the row flips to "Downloaded".
                  void get().loadStatus()
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
                    // Surface the failure ONLY via the per-version progress
                    // message (which carries version/flavor context); setting
                    // the global `s.error` here too produced a duplicate
                    // top-level Alert. Connection/load errors still use s.error.
                  })
                },
                error: (msg: string) => {
                  set(s => {
                    s.sseConnected = false
                    s.error = msg
                  })
                },
                default: (_event: string, _data: unknown) => {},
              },
            } as any,
          )
        } catch (e) {
          // Audit Net3: server bounce or 503 cap-rejection. Mirror
          // the LlmModelDownload reconnect pattern: bounded attempts,
          // fixed delay, give up + surface error after max.
          sseController = null
          set(s => {
            s.sseConnected = false
          })
          sseReconnectAttempts += 1
          if (sseReconnectAttempts < SSE_MAX_RECONNECT_ATTEMPTS) {
            set(s => {
              s.error = `SSE disconnected; reconnecting (attempt ${sseReconnectAttempts}/${SSE_MAX_RECONNECT_ATTEMPTS})`
            })
            sseReconnectTimer = setTimeout(() => {
              sseReconnectTimer = null
              void get().subscribeToInstallProgress()
            }, SSE_RECONNECT_DELAY_MS)
          } else {
            set(s => {
              s.error =
                e instanceof Error
                  ? `SSE failed after ${SSE_MAX_RECONNECT_ATTEMPTS} attempts: ${e.message}`
                  : `SSE failed after ${SSE_MAX_RECONNECT_ATTEMPTS} attempts`
            })
            sseReconnectAttempts = 0
          }
        }
      },
    })),
  ),
)
