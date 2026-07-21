/**
 * Tier 1 — Updater Zustand store unit tests.
 *
 * Mocks `@/api-client`'s `ApiClient.Updater.*` to assert:
 *   - loadStatus copies the status fields + stops polling when settled
 *   - check populates available/version/notes + clears error
 *   - download calls the endpoint and starts a (single) poll timer
 *   - install calls the endpoint
 *   - startPolling/stopPolling are idempotent (one timer)
 *   - error paths capture + surface messages
 */

import { beforeEach, describe, expect, it, vi } from 'vitest'

const apiMock = vi.hoisted(() => ({
  Updater: {
    check: vi.fn(),
    download: vi.fn(),
    install: vi.fn(),
    status: vi.fn(),
  },
}))

vi.mock('@/api-client', () => ({
  ApiClient: apiMock,
}))

// getVersion is only used by __init__ (not exercised here); mock it so the
// import resolves cleanly in the jsdom/test environment.
vi.mock('@tauri-apps/api/app', () => ({
  getVersion: vi.fn().mockResolvedValue('1.2.3'),
}))

import { useUpdaterStore } from '@ziee/desktop/modules/updater/stores/updater'

function statusPayload(overrides = {}) {
  return {
    status: {
      checking: false,
      available: false,
      downloading: false,
      ready_to_install: false,
      version: null,
      notes: null,
      progress: null,
      error: null,
      ...overrides,
    },
  }
}

beforeEach(() => {
  useUpdaterStore.getState().stopPolling()
  useUpdaterStore.getState().stopDailyChecks()
  // Cast to any: lazy-action dispatchers widen the Zustand state type so
  // TypeScript refuses { checking: false } as Partial<FullStoreState>.
  ;(useUpdaterStore.setState as any)({
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
  })
  for (const v of Object.values(apiMock.Updater)) {
    if (typeof v === 'function' && 'mockReset' in v) v.mockReset()
  }
})

describe('UpdaterStore', () => {
  describe('loadStatus', () => {
    it('copies status fields and stops polling when not downloading', async () => {
      apiMock.Updater.status.mockResolvedValueOnce(
        statusPayload({ available: true, version: '2.0.0', notes: 'notes', ready_to_install: true }),
      )

      await useUpdaterStore.getState().loadStatus()

      const s = useUpdaterStore.getState()
      expect(s.available).toBe(true)
      expect(s.version).toBe('2.0.0')
      expect(s.notes).toBe('notes')
      expect(s.readyToInstall).toBe(true)
      expect(s.downloading).toBe(false)
      expect(s.pollTimer).toBeNull()
    })

    it('captures error message on failure', async () => {
      apiMock.Updater.status.mockRejectedValueOnce(new Error('status boom'))

      await useUpdaterStore.getState().loadStatus()

      expect(useUpdaterStore.getState().error).toBe('status boom')
    })
  })

  describe('check', () => {
    it('populates available/version/notes and clears checking', async () => {
      apiMock.Updater.check.mockResolvedValueOnce({
        available: true,
        version: '3.1.4',
        notes: 'shiny',
      })

      await useUpdaterStore.getState().check()

      const s = useUpdaterStore.getState()
      expect(apiMock.Updater.check).toHaveBeenCalledWith(undefined, undefined)
      expect(s.available).toBe(true)
      expect(s.version).toBe('3.1.4')
      expect(s.notes).toBe('shiny')
      expect(s.checking).toBe(false)
      expect(s.error).toBeNull()
    })

    it('captures error and clears checking on failure', async () => {
      apiMock.Updater.check.mockRejectedValueOnce(new Error('no network'))

      await useUpdaterStore.getState().check()

      const s = useUpdaterStore.getState()
      expect(s.error).toBe('no network')
      expect(s.checking).toBe(false)
    })
  })

  describe('download', () => {
    it('calls the endpoint, marks downloading, and starts a poll timer', async () => {
      apiMock.Updater.download.mockResolvedValueOnce({ success: true, message: 'started' })
      // The poll tick calls status(); keep it resolving so a stray tick is harmless.
      apiMock.Updater.status.mockResolvedValue(statusPayload({ downloading: true, progress: 10 }))

      await useUpdaterStore.getState().download()

      const s = useUpdaterStore.getState()
      expect(apiMock.Updater.download).toHaveBeenCalledWith(undefined, undefined)
      expect(s.downloading).toBe(true)
      expect(s.pollTimer).not.toBeNull()

      // Cleanup so the interval doesn't leak into other tests.
      useUpdaterStore.getState().stopPolling()
    })

    it('captures error and clears downloading when the request fails', async () => {
      apiMock.Updater.download.mockRejectedValueOnce(new Error('start failed'))

      await useUpdaterStore.getState().download()

      const s = useUpdaterStore.getState()
      expect(s.error).toBe('start failed')
      expect(s.downloading).toBe(false)
      expect(s.pollTimer).toBeNull()
    })
  })

  describe('install', () => {
    it('calls the install endpoint', async () => {
      apiMock.Updater.install.mockResolvedValueOnce({ success: true, message: 'installing' })

      await useUpdaterStore.getState().install()

      expect(apiMock.Updater.install).toHaveBeenCalledWith(undefined, undefined)
    })
  })

  describe('installAndRestart (sidebar card one-click flow)', () => {
    it('starts the download, flags autoInstall, and begins polling', async () => {
      apiMock.Updater.download.mockResolvedValueOnce({ success: true, message: 'started' })
      apiMock.Updater.status.mockResolvedValue(statusPayload({ downloading: true, progress: 5 }))

      await useUpdaterStore.getState().installAndRestart()

      const s = useUpdaterStore.getState()
      expect(apiMock.Updater.download).toHaveBeenCalledWith(undefined, undefined)
      expect(s.downloading).toBe(true)
      expect(s.autoInstall).toBe(true)
      expect(s.pollTimer).not.toBeNull()

      useUpdaterStore.getState().stopPolling()
    })

    it('auto-installs once the download is ready (poll sees ready_to_install)', async () => {
      // Simulate being mid-install-flow: autoInstall set, then a poll tick
      // reports the bytes are ready.
      ;(useUpdaterStore.setState as any)({ autoInstall: true, downloading: true })
      apiMock.Updater.status.mockResolvedValueOnce(
        statusPayload({ downloading: false, ready_to_install: true, progress: 100 }),
      )
      apiMock.Updater.install.mockResolvedValueOnce({ success: true, message: 'installing' })

      await useUpdaterStore.getState().loadStatus()

      expect(apiMock.Updater.install).toHaveBeenCalledWith(undefined, undefined)
      // autoInstall consumed so we don't re-install on a later tick.
      expect(useUpdaterStore.getState().autoInstall).toBe(false)
    })

    it('does NOT auto-install when autoInstall is not set', async () => {
      apiMock.Updater.status.mockResolvedValueOnce(
        statusPayload({ downloading: false, ready_to_install: true, progress: 100 }),
      )

      await useUpdaterStore.getState().loadStatus()

      expect(apiMock.Updater.install).not.toHaveBeenCalled()
    })

    it('clears downloading/autoInstall and captures error if the download request fails', async () => {
      apiMock.Updater.download.mockRejectedValueOnce(new Error('offline'))

      await useUpdaterStore.getState().installAndRestart()

      const s = useUpdaterStore.getState()
      expect(s.downloading).toBe(false)
      expect(s.autoInstall).toBe(false)
      expect(s.error).toBe('offline')
    })
  })

  describe('remindLater', () => {
    it('dismisses the card for this session', () => {
      ;(useUpdaterStore.setState as any)({ available: true, dismissed: false })

      useUpdaterStore.getState().remindLater()

      expect(useUpdaterStore.getState().dismissed).toBe(true)
    })
  })

  describe('daily background check', () => {
    it('check({ resurface: true }) un-dismisses a previously hidden update', async () => {
      ;(useUpdaterStore.setState as any)({ dismissed: true })
      apiMock.Updater.check.mockResolvedValueOnce({ available: true, version: '2.0.0', notes: '' })

      await useUpdaterStore.getState().check({ resurface: true })

      expect(useUpdaterStore.getState().dismissed).toBe(false)
    })

    it('a plain check() does NOT un-dismiss', async () => {
      ;(useUpdaterStore.setState as any)({ dismissed: true })
      apiMock.Updater.check.mockResolvedValueOnce({ available: true, version: '2.0.0', notes: '' })

      await useUpdaterStore.getState().check()

      expect(useUpdaterStore.getState().dismissed).toBe(true)
    })

    it('resurface only un-dismisses when an update is actually available', async () => {
      ;(useUpdaterStore.setState as any)({ dismissed: true })
      apiMock.Updater.check.mockResolvedValueOnce({ available: false, version: null, notes: null })

      await useUpdaterStore.getState().check({ resurface: true })

      expect(useUpdaterStore.getState().dismissed).toBe(true)
    })

    it('startDailyChecks is idempotent and stop clears the timer', () => {
      const s = useUpdaterStore.getState()
      s.startDailyChecks()
      const t1 = useUpdaterStore.getState().dailyTimer
      s.startDailyChecks()
      const t2 = useUpdaterStore.getState().dailyTimer
      expect(t1).toBe(t2)
      expect(t1).not.toBeNull()
      s.stopDailyChecks()
      expect(useUpdaterStore.getState().dailyTimer).toBeNull()
    })

    it('fires a re-check ~once a day while open', async () => {
      vi.useFakeTimers()
      try {
        apiMock.Updater.check.mockResolvedValue({ available: false, version: null, notes: null })
        useUpdaterStore.getState().startDailyChecks()
        expect(apiMock.Updater.check).not.toHaveBeenCalled()

        await vi.advanceTimersByTimeAsync(24 * 60 * 60 * 1000)
        expect(apiMock.Updater.check).toHaveBeenCalledTimes(1)

        await vi.advanceTimersByTimeAsync(24 * 60 * 60 * 1000)
        expect(apiMock.Updater.check).toHaveBeenCalledTimes(2)
      } finally {
        useUpdaterStore.getState().stopDailyChecks()
        vi.useRealTimers()
      }
    })

    it('skips the daily re-check while a download/install is in progress', async () => {
      vi.useFakeTimers()
      try {
        ;(useUpdaterStore.setState as any)({ downloading: true })
        apiMock.Updater.check.mockResolvedValue({ available: false, version: null, notes: null })
        useUpdaterStore.getState().startDailyChecks()

        await vi.advanceTimersByTimeAsync(24 * 60 * 60 * 1000)
        expect(apiMock.Updater.check).not.toHaveBeenCalled()
      } finally {
        useUpdaterStore.getState().stopDailyChecks()
        vi.useRealTimers()
      }
    })
  })

  describe('startPolling / stopPolling', () => {
    it('is idempotent — second start does not double up the timer', () => {
      apiMock.Updater.status.mockResolvedValue(statusPayload())
      const s = useUpdaterStore.getState()
      s.startPolling()
      const t1 = useUpdaterStore.getState().pollTimer
      s.startPolling()
      const t2 = useUpdaterStore.getState().pollTimer
      expect(t1).toBe(t2)
      s.stopPolling()
    })

    it('clears the timer on stop', () => {
      const s = useUpdaterStore.getState()
      s.startPolling()
      expect(useUpdaterStore.getState().pollTimer).not.toBeNull()
      s.stopPolling()
      expect(useUpdaterStore.getState().pollTimer).toBeNull()
    })
  })
})
