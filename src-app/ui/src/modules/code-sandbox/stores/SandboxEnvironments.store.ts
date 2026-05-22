import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { immer } from 'zustand/middleware/immer'
import { ApiClient } from '@/api-client'
import type {
  EnvironmentInfo,
  FetchPhase,
  SSEPrefetchCompleteData,
  SSEPrefetchConnectedData,
  SSEPrefetchFailedData,
  SSEPrefetchProgressData,
} from '@/api-client/types'

/** Live progress for an in-flight (or just-finished) prefetch, keyed by flavor. */
interface FlavorProgress {
  status: 'running' | 'completed' | 'failed'
  phase?: FetchPhase
  message?: string
  bytesDownloaded?: number
  error?: string
}

interface SandboxEnvironmentsStore {
  environments: EnvironmentInfo[]
  loading: boolean
  error: string | null
  progress: Record<string, FlavorProgress>
  /** Per-flavor in-flight evict flag (drives the Evict button's loading state). */
  evicting: Record<string, boolean>

  __init__: {
    environments?: () => Promise<void>
  }

  loadEnvironments: () => Promise<void>
  resumeRunningTasks: () => Promise<void>
  startPrefetch: (flavor: string) => Promise<void>
  subscribeToEvents: (flavor: string) => Promise<void>
  evictEnvironment: (flavor: string) => Promise<void>
}

// Per-flavor abort controllers for SSE cleanup. Module-scoped (not in
// immer state — AbortController isn't draftable). Doubles as a guard
// against double-subscribing the same flavor.
const sseControllers: Record<string, AbortController> = {}

function cleanupSse(flavor: string) {
  sseControllers[flavor]?.abort()
  delete sseControllers[flavor]
}

export const useSandboxEnvironmentsStore = create<SandboxEnvironmentsStore>()(
  subscribeWithSelector(
    immer((set, get) => ({
      environments: [],
      loading: false,
      error: null,
      progress: {},
      evicting: {},

      __init__: {
        // On first access: load the flavor list AND reconcile with any
        // server-side running tasks (page-reload resilience).
        environments: async () => {
          await get().loadEnvironments()
          await get().resumeRunningTasks()
        },
      },

      loadEnvironments: async () => {
        set(s => {
          s.loading = true
          s.error = null
        })
        try {
          const res = await ApiClient.CodeSandbox.listEnvironments(undefined)
          set(s => {
            s.environments = res.available
            s.loading = false
          })
        } catch (e: any) {
          set(s => {
            s.error = e?.message ?? 'Failed to load environments'
            s.loading = false
          })
        }
      },

      // On (re)load: ask the server which tasks are running and re-attach
      // an SSE stream to each. The backend's SSE handler replays buffered
      // progress on subscribe, so the bar resumes from the current phase —
      // even if this browser never started the fetch (another admin did).
      resumeRunningTasks: async () => {
        try {
          const res = await ApiClient.CodeSandbox.listPrefetchTasks(undefined)
          for (const t of res.tasks) {
            if (t.status === 'running' && !sseControllers[t.flavor]) {
              set(s => {
                s.progress[t.flavor] = {
                  status: 'running',
                  phase: t.last_phase ?? undefined,
                }
              })
              void get().subscribeToEvents(t.flavor)
            }
          }
        } catch {
          // listPrefetchTasks failing (e.g. 403) is non-fatal — the page
          // still shows cached status from /environments.
        }
      },

      startPrefetch: async (flavor: string) => {
        await ApiClient.CodeSandbox.startPrefetch({ flavor })
        set(s => {
          s.progress[flavor] = { status: 'running' }
        })
        await get().subscribeToEvents(flavor)
      },

      subscribeToEvents: async (flavor: string) => {
        if (sseControllers[flavor]) return // already streaming
        await ApiClient.CodeSandbox.subscribePrefetchEvents(
          { flavor },
          {
            SSE: {
              __init: ({
                abortController,
              }: {
                abortController: AbortController
              }) => {
                sseControllers[flavor] = abortController
              },
              connected: (_d: SSEPrefetchConnectedData) => {},
              progress: (d: SSEPrefetchProgressData) => {
                set(s => {
                  s.progress[flavor] = {
                    ...(s.progress[flavor] ?? { status: 'running' }),
                    status: 'running',
                    phase: d.phase,
                    message: d.message,
                  }
                })
              },
              complete: (d: SSEPrefetchCompleteData) => {
                set(s => {
                  s.progress[flavor] = {
                    status: 'completed',
                    bytesDownloaded: d.bytes_downloaded,
                  }
                })
                cleanupSse(flavor)
                void get().loadEnvironments() // cached flag flips
              },
              failed: (d: SSEPrefetchFailedData) => {
                set(s => {
                  s.progress[flavor] = { status: 'failed', error: d.error }
                })
                cleanupSse(flavor)
              },
              default: (_event: string, _data: unknown) => {},
            },
          } as any,
        )
      },

      evictEnvironment: async (flavor: string) => {
        set(s => {
          s.evicting[flavor] = true
          s.error = null
        })
        try {
          // The endpoint returns the refreshed environments list (cached flips
          // false). Also clear any stale progress so the row resets cleanly.
          const res = await ApiClient.CodeSandbox.evictEnvironment({ flavor })
          cleanupSse(flavor)
          set(s => {
            s.environments = res.available
            delete s.progress[flavor]
            delete s.evicting[flavor]
          })
        } catch (e: any) {
          set(s => {
            s.error = e?.message ?? 'Failed to evict environment'
            delete s.evicting[flavor]
          })
        }
      },
    })),
  ),
)
