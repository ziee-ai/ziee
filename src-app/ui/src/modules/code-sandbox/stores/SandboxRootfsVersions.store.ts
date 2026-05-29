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
    rootfsVersions?: () => Promise<void>
  }
  __destroy__?: () => void

  loadStatus: () => Promise<void>
  installVersion: (
    version: string,
    arch: string,
    flavor: string,
    pkg: string,
  ) => Promise<void>
  setPin: (version: string) => Promise<void>
  deleteArtifact: (id: string) => Promise<void>
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
      lastSwap: null,
      loading: false,
      error: null,
      actions: {},
      installTasks: {},
      sseConnected: false,

      __init__: {
        sandboxRootfsVersions: async () => {
          await get().loadStatus()
          // Open the SSE channel for live install progress. Idempotent
          // via the sseController guard.
          void get().subscribeToInstallProgress()
        },
      },

      __destroy__: () => {
        cleanupSse()
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
            s.draining = res.draining
            s.conversationCount = res.conversation_count
            s.mcpServerWorkspaceCount = res.mcp_server_workspace_count
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
            s.draining = res.draining
            s.conversationCount = res.conversation_count
            s.mcpServerWorkspaceCount = res.mcp_server_workspace_count
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
                    }
                  })
                },
                progress: (d: SSEInstallProgressData) => {
                  set(s => {
                    // Walk active tasks to find the matching task_id.
                    for (const key of Object.keys(s.installTasks)) {
                      const t = s.installTasks[key]
                      if (t.task_id === d.task_id) {
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
                    s.error = d.error
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
