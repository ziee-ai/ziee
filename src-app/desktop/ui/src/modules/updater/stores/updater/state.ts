import type { StoreSet } from '@ziee/framework/store-kit'
import { type StoreProxy } from '@ziee/framework/stores'

declare module '@ziee/framework/stores' {
  interface RegisteredStores {
    Updater: StoreProxy<UpdaterState>
  }
}

export interface UpdaterActions {
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

export const updaterState = {
  /** The running app version (from Tauri). */
  currentVersion: null as string | null,
  checking: false,
  available: false,
  downloading: false,
  readyToInstall: false,
  /** 0-100 while downloading, else null. */
  progress: null as number | null,
  /** Version of the AVAILABLE update (null until a check finds one). */
  version: null as string | null,
  notes: null as string | null,
  error: null as string | null,
  /** "Remind later" hides the sidebar card this session (in-memory). */
  dismissed: false,
  /** Auto-install the moment a download is ready (installAndRestart flow). */
  autoInstall: false,
  /** Polling timer handle while a download is in flight. */
  pollTimer: null as ReturnType<typeof setInterval> | null,
  /** Daily background re-check timer (active while the app is open). */
  dailyTimer: null as ReturnType<typeof setInterval> | null,
}

export type UpdaterState = typeof updaterState & UpdaterActions
export type UpdaterSet = StoreSet<UpdaterState>
export type UpdaterGet = () => UpdaterState
