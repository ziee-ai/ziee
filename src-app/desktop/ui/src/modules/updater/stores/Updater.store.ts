/**
 * Auto-Updater Zustand store (desktop only).
 *
 * Wraps the typed `ApiClient.Updater.*` methods (generated from the desktop
 * crate's aide-documented `/api/desktop/updater/*` routes) so the About page
 * only deals with `Stores.Updater.check()` etc., never raw fetches.
 *
 * The backend downloads in a spawned task and exposes progress via
 * `GET /status`; this store polls `status` while a download is in flight
 * (same idempotent-timer pattern as RemoteAccess's magic-link rotation).
 */

import { create } from 'zustand'
import { immer } from 'zustand/middleware/immer'
import { subscribeWithSelector } from 'zustand/middleware'
import { getVersion } from '@tauri-apps/api/app'
import { ApiClient } from '@/api-client'
import { type StoreProxy } from '@/core/stores'

// Poll cadence while downloading. Comfortably frequent for a smooth
// progress bar without hammering the local server.
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

  /** "Remind later" hides the sidebar card for this session. Resets on
   *  next launch (in-memory), so the card reappears. */
  dismissed: boolean
  /** Set while an `installAndRestart` flow is in progress so the poll loop
   *  auto-installs the moment the download is ready. */
  autoInstall: boolean

  /** Polling timer handle while a download is in flight. */
  pollTimer: ReturnType<typeof setInterval> | null
  /** Daily background re-check timer (active while the app is open). */
  dailyTimer: ReturnType<typeof setInterval> | null

  // Key MUST be a state property the surfaces read — the store proxy only fires
  // `__init__[prop]` on first access of `prop`. The About page and the sidebar
  // card both read `available`, so it hydrates for either.
  __init__: {
    available: () => Promise<void>
  }
  __destroy__: () => void

  loadStatus: () => Promise<void>
  /** Query the updater endpoint. `resurface: true` (used by the daily
   *  background check) un-dismisses the card when an update is found, so a
   *  previously "Remind later"-ed update reappears the next day. */
  check: (opts?: { resurface?: boolean }) => Promise<void>
  /** Start/stop the once-a-day background re-check loop. */
  startDailyChecks: () => void
  stopDailyChecks: () => void
  download: () => Promise<void>
  install: () => Promise<void>
  /** One-click flow for the sidebar card: download, then auto-install +
   *  restart as soon as the bytes are ready. Progress is observable via
   *  `progress`/`downloading`. */
  installAndRestart: () => Promise<void>
  /** "Remind later" — hide the card this session. */
  remindLater: () => void
  startPolling: () => void
  stopPolling: () => void
}

declare module '@/core/stores' {
  interface RegisteredStores {
    Updater: StoreProxy<UpdaterState>
  }
}

const updaterClient = ApiClient.Updater

export const useUpdaterStore = create<UpdaterState>()(
  subscribeWithSelector(
    immer((set, get) => ({
      currentVersion: null,
      checking: false,
      available: false,
      downloading: false,
      readyToInstall: false,
      progress: null,
      version: null,
      notes: null,
      error: null,
      dismissed: false,
      autoInstall: false,
      pollTimer: null,
      dailyTimer: null,

      __init__: {
        available: async () => {
          // Record the running version, then silently check for an update so
          // the sidebar card can surface one without any OS notification.
          try {
            const v = await getVersion()
            set((s) => {
              s.currentVersion = v
            })
          } catch {
            // Non-Tauri context (e.g. unit test) — leave version null.
          }
          await get().check()
          // Keep checking once a day while the app stays open.
          get().startDailyChecks()
        },
      },

      __destroy__: () => {
        get().stopPolling()
        get().stopDailyChecks()
      },

      loadStatus: async () => {
        try {
          const { status } = await updaterClient.status(undefined, undefined)
          set((s) => {
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
          if (!status.downloading) {
            get().stopPolling()
          }
          // One-click flow: the bytes are ready → install + restart now.
          if (status.ready_to_install && get().autoInstall) {
            set((s) => {
              s.autoInstall = false
            })
            await get().install()
          }
        } catch (e) {
          set((s) => {
            s.error = e instanceof Error ? e.message : 'Failed to load update status'
          })
        }
      },

      check: async (opts) => {
        set((s) => {
          s.checking = true
          s.error = null
        })
        try {
          const res = await updaterClient.check(undefined, undefined)
          set((s) => {
            s.checking = false
            s.available = res.available
            s.version = res.version ?? null
            s.notes = res.notes ?? null
            // The daily background check re-surfaces a previously dismissed
            // update ("Remind later" → remind me the next day).
            if (opts?.resurface && res.available) {
              s.dismissed = false
            }
          })
        } catch (e) {
          set((s) => {
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
        set((s) => {
          s.dailyTimer = timer
        })
      },

      stopDailyChecks: () => {
        const timer = get().dailyTimer
        if (timer) {
          clearInterval(timer)
          set((s) => {
            s.dailyTimer = null
          })
        }
      },

      download: async () => {
        set((s) => {
          s.downloading = true
          s.progress = 0
          s.error = null
        })
        try {
          await updaterClient.download(undefined, undefined)
          get().startPolling()
        } catch (e) {
          set((s) => {
            s.downloading = false
            s.error = e instanceof Error ? e.message : 'Download failed to start'
          })
        }
      },

      install: async () => {
        set((s) => {
          s.error = null
        })
        try {
          // The backend quits + restarts the app on success, so we don't
          // expect to handle a meaningful response here.
          await updaterClient.install(undefined, undefined)
        } catch (e) {
          set((s) => {
            s.error = e instanceof Error ? e.message : 'Install failed'
          })
        }
      },

      installAndRestart: async () => {
        set((s) => {
          s.downloading = true
          s.progress = 0
          s.autoInstall = true
          s.error = null
        })
        try {
          await updaterClient.download(undefined, undefined)
          // The poll loop watches `ready_to_install` and, because
          // `autoInstall` is set, calls `install()` (which restarts the app)
          // the moment the bytes land.
          get().startPolling()
        } catch (e) {
          set((s) => {
            s.downloading = false
            s.autoInstall = false
            s.error = e instanceof Error ? e.message : 'Update failed to start'
          })
        }
      },

      remindLater: () => {
        // Hide the card for this session; it reappears on the next launch's
        // silent check.
        set((s) => {
          s.dismissed = true
        })
      },

      startPolling: () => {
        if (get().pollTimer) return // idempotent — single timer
        const timer = setInterval(() => {
          void get().loadStatus()
        }, POLL_INTERVAL_MS)
        set((s) => {
          s.pollTimer = timer
        })
      },

      stopPolling: () => {
        const timer = get().pollTimer
        if (timer) {
          clearInterval(timer)
          set((s) => {
            s.pollTimer = null
          })
        }
      },
    })),
  ),
)
