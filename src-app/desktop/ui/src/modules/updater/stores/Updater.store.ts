/**
 * Auto-Updater store (desktop only). Wraps `ApiClient.Updater.*` so the About
 * page only deals with `Stores.Updater.check()` etc. The backend downloads in a
 * spawned task and exposes progress via `GET /status`; this store polls `status`
 * while a download is in flight (same idempotent-timer pattern as RemoteAccess).
 */

import { getVersion } from '@tauri-apps/api/app'
import { ApiClient } from '@/api-client'
import { defineStore } from '@ziee/framework/store-kit'
import { type StoreProxy } from '@ziee/framework/stores'

// Poll cadence while downloading (smooth progress bar without hammering).
const POLL_INTERVAL_MS = 800
// Background re-check cadence while the app stays open (once a day).
const DAILY_CHECK_INTERVAL_MS = 24 * 60 * 60 * 1000

interface UpdaterState {
  /** The running app version (from Tauri). */
  currentVersion: string | null
  checking: boolean
  available: boolean
  downloading: boolean
  readyToInstall: boolean
  /** 0-100 while downloading, else null. */
  progress: number | null
  /** Version of the AVAILABLE update (null until a check finds one). */
  version: string | null
  notes: string | null
  error: string | null
  /** "Remind later" hides the sidebar card this session (in-memory). */
  dismissed: boolean
  /** Auto-install the moment a download is ready (installAndRestart flow). */
  autoInstall: boolean
  /** Polling timer handle while a download is in flight. */
  pollTimer: ReturnType<typeof setInterval> | null
  /** Daily background re-check timer (active while the app is open). */
  dailyTimer: ReturnType<typeof setInterval> | null
  loadStatus: () => Promise<void>
  check: (opts?: { resurface?: boolean }) => Promise<void>
  startDailyChecks: () => void
  stopDailyChecks: () => void
  download: () => Promise<void>
  install: () => Promise<void>
  installAndRestart: () => Promise<void>
  remindLater: () => void
  startPolling: () => void
  stopPolling: () => void
}

declare module '@ziee/framework/stores' {
  interface RegisteredStores {
    Updater: StoreProxy<UpdaterState>
  }
}

const updaterClient = ApiClient.Updater

export const Updater = defineStore('Updater', {
  immer: true,
  state: {
    currentVersion: null as string | null,
    checking: false,
    available: false,
    downloading: false,
    readyToInstall: false,
    progress: null as number | null,
    version: null as string | null,
    notes: null as string | null,
    error: null as string | null,
    dismissed: false,
    autoInstall: false,
    pollTimer: null as ReturnType<typeof setInterval> | null,
    dailyTimer: null as ReturnType<typeof setInterval> | null,
  },
  actions: (set, getRaw) => {
    const get = getRaw as () => UpdaterState
    return {
      loadStatus: async () => {
        try {
          const { status } = await updaterClient.status(undefined, undefined)
          set(s => {
            s.checking = status.checking
            s.available = status.available
            s.downloading = status.downloading
            s.readyToInstall = status.ready_to_install
            s.progress = status.progress ?? null
            s.version = status.version ?? null
            s.notes = status.notes ?? null
            s.error = status.error ?? null
          })
          // Stop polling once the download settled (ready or errored).
          if (!status.downloading) get().stopPolling()
          // One-click flow: bytes ready → install + restart now.
          if (status.ready_to_install && get().autoInstall) {
            set(s => {
              s.autoInstall = false
            })
            await get().install()
          }
        } catch (e) {
          set(s => {
            s.error = e instanceof Error ? e.message : 'Failed to load update status'
          })
        }
      },
      check: async (opts?: { resurface?: boolean }) => {
        set(s => {
          s.checking = true
          s.error = null
        })
        try {
          const res = await updaterClient.check(undefined, undefined)
          set(s => {
            s.checking = false
            s.available = res.available
            s.version = res.version ?? null
            s.notes = res.notes ?? null
            // The daily background check re-surfaces a dismissed update.
            if (opts?.resurface && res.available) s.dismissed = false
          })
        } catch (e) {
          set(s => {
            s.checking = false
            s.error = e instanceof Error ? e.message : 'Update check failed'
          })
        }
      },
      startDailyChecks: () => {
        if (get().dailyTimer) return // idempotent — single timer
        const timer = setInterval(() => {
          const s = get()
          // Don't disturb an in-progress download/install.
          if (s.downloading || s.readyToInstall) return
          void s.check({ resurface: true })
        }, DAILY_CHECK_INTERVAL_MS)
        set(s => {
          s.dailyTimer = timer
        })
      },
      stopDailyChecks: () => {
        const timer = get().dailyTimer
        if (timer) {
          clearInterval(timer)
          set(s => {
            s.dailyTimer = null
          })
        }
      },
      download: async () => {
        set(s => {
          s.downloading = true
          s.progress = 0
          s.error = null
        })
        try {
          await updaterClient.download(undefined, undefined)
          get().startPolling()
        } catch (e) {
          set(s => {
            s.downloading = false
            s.error = e instanceof Error ? e.message : 'Download failed to start'
          })
        }
      },
      install: async () => {
        set(s => {
          s.error = null
        })
        try {
          // The backend quits + restarts on success, so no meaningful response.
          await updaterClient.install(undefined, undefined)
        } catch (e) {
          set(s => {
            s.error = e instanceof Error ? e.message : 'Install failed'
          })
        }
      },
      installAndRestart: async () => {
        set(s => {
          s.downloading = true
          s.progress = 0
          s.autoInstall = true
          s.error = null
        })
        try {
          await updaterClient.download(undefined, undefined)
          // The poll loop watches `ready_to_install` and, with autoInstall set,
          // calls install() (which restarts) the moment the bytes land.
          get().startPolling()
        } catch (e) {
          set(s => {
            s.downloading = false
            s.autoInstall = false
            s.error = e instanceof Error ? e.message : 'Update failed to start'
          })
        }
      },
      remindLater: () => {
        // Hide the card this session; it reappears on the next launch's check.
        set(s => {
          s.dismissed = true
        })
      },
      startPolling: () => {
        if (get().pollTimer) return // idempotent — single timer
        const timer = setInterval(() => {
          void get().loadStatus()
        }, POLL_INTERVAL_MS)
        set(s => {
          s.pollTimer = timer
        })
      },
      stopPolling: () => {
        const timer = get().pollTimer
        if (timer) {
          clearInterval(timer)
          set(s => {
            s.pollTimer = null
          })
        }
      },
    }
  },
  init: ({ set, actions, onCleanup }) => {
    // Record the running version, then silently check for an update so the
    // sidebar card can surface one; keep checking once a day while open.
    void (async () => {
      try {
        const v = await getVersion()
        set(s => {
          s.currentVersion = v
        })
      } catch {
        // Non-Tauri context (e.g. unit test) — leave version null.
      }
      await actions.check()
      actions.startDailyChecks()
    })()
    onCleanup(() => {
      actions.stopPolling()
      actions.stopDailyChecks()
    })
  },
})

export const useUpdaterStore = Updater.store
