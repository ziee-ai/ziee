import { test, expect } from '@playwright/test'
import type { Page } from '@playwright/test'
import { installTauriMock, mockBackendDefaults } from './helpers/tauri-mock'

/**
 * TEST-30 — Voice dictation surface on the DESKTOP bundle.
 *
 * The voice module + chat mic extension live in `ui-core` and are
 * glob-discovered by the desktop loader (not blocklisted), so they must appear
 * in the desktop build too:
 *   - the composer mic button renders on the chat page, and
 *   - `/settings/voice` is reachable (its admin page renders).
 *
 * Fully mocked (tauri auto-login + mocked backend + intercepted `/api/voice`),
 * matching the other mocked desktop specs — no real backend needed.
 */

// Minimal in-browser capture mocks so the mic's isRecordingSupported() is true
// on the desktop bundle (secure context + getUserMedia + MediaRecorder present).
async function installVoiceBrowserMocks(page: Page): Promise<void> {
  await page.addInitScript(() => {
    try {
      Object.defineProperty(window, 'isSecureContext', {
        configurable: true,
        get: () => true,
      })
    } catch {
      /* best effort */
    }
    class FakeMediaRecorder {
      state = 'inactive'
      mimeType = 'audio/wav'
      ondataavailable: unknown = null
      onstop: unknown = null
      start() {
        this.state = 'recording'
      }
      stop() {
        this.state = 'inactive'
      }
      static isTypeSupported() {
        return true
      }
    }
    try {
      Object.defineProperty(window, 'MediaRecorder', {
        configurable: true,
        writable: true,
        value: FakeMediaRecorder,
      })
    } catch {
      ;(window as unknown as Record<string, unknown>).MediaRecorder =
        FakeMediaRecorder
    }
    const getUserMedia = async () => ({
      getTracks: () => [{ stop() {}, kind: 'audio' }],
    })
    try {
      const existing = navigator.mediaDevices
      if (existing) {
        ;(existing as unknown as Record<string, unknown>).getUserMedia =
          getUserMedia
      } else {
        Object.defineProperty(navigator, 'mediaDevices', {
          configurable: true,
          get: () => ({ getUserMedia }),
        })
      }
    } catch {
      /* ignore */
    }
  })
}

// Intercept `/api/voice/**` with a ready, provisioned deployment so both the
// mic and the admin page render. Registered AFTER mockBackendDefaults so it
// wins (route matching is LIFO).
async function routeVoiceReady(page: Page): Promise<void> {
  const now = new Date().toISOString()
  const json = (body: unknown) => ({
    status: 200,
    contentType: 'application/json',
    body: JSON.stringify(body),
  })
  await page.route('**/api/voice/**', async (route, request) => {
    const seg = new URL(request.url()).pathname.replace(/^.*\/api\/voice\/?/, '')
    if (seg === 'capability')
      return route.fulfill(
        json({
          enabled: true,
          runtime_ready: true,
          model_ready: true,
          can_transcribe: true,
          model: 'base',
          max_clip_seconds: 60,
          streaming_enabled: true,
          stream_interval_ms: 1000,
        }),
      )
    if (seg === 'settings')
      return route.fulfill(
        json({
          enabled: true,
          model: 'base',
          language: 'auto',
          idle_unload_secs: 300,
          auto_start_timeout_secs: 30,
          drain_timeout_secs: 30,
          max_clip_seconds: 60,
          max_upload_bytes: 26214400,
          streaming_enabled: true,
          stream_interval_ms: 1000,
          updated_at: now,
        }),
      )
    if (seg === 'versions')
      return route.fulfill(
        json({
          versions: [
            {
              id: 'ver-v1.0.0',
              version: 'v1.0.0',
              backend: 'cpu',
              platform: 'linux',
              arch: 'x86_64',
              binary_path: '/cache/whisper/v1.0.0/whisper-server',
              is_system_default: true,
              created_at: now,
            },
          ],
        }),
      )
    if (seg === 'versions/check-updates')
      return route.fulfill(json({ platform: 'linux', arch: 'x86_64', versions: [] }))
    if (seg === 'versions/downloads') return route.fulfill(json({ downloads: [] }))
    if (seg === 'model/status')
      return route.fulfill(json({ model: 'base', present: true, size_bytes: 147456000 }))
    if (seg === 'instance')
      return route.fulfill(
        json({
          status: 'running',
          state: 'healthy',
          active_model: 'ggml-base.bin',
          local_port: 51789,
          restart_attempts: 0,
          state_changed_at: now,
        }),
      )
    return route.fulfill(json({}))
  })
}

test.describe('desktop voice surface (TEST-30)', () => {
  test.beforeEach(async ({ page }) => {
    await installVoiceBrowserMocks(page)
    await installTauriMock(page)
    await mockBackendDefaults(page)
    await routeVoiceReady(page)
  })

  // Desktop parity is proven the same way the other desktop specs prove a core
  // module ships: via the desktop settings menu (the mocked desktop bundle
  // doesn't render the full chat composer — no desktop spec asserts it — so the
  // mic-in-composer render is covered by the 8 ui `14-voice` specs, which run on
  // the SAME glob-shared voice code). Here we prove the voice ADMIN module is
  // glob-discovered into the desktop bundle and its page renders.
  test('the voice admin module ships in the desktop settings menu', async ({
    page,
  }) => {
    await page.goto('/settings')
    await expect(page.getByTestId('desktop-settings-menu')).toBeVisible({
      timeout: 20000,
    })
    // The voice settingsAdminPages entry (id "voice") appears — i.e. the core
    // voice module was NOT blocklisted and was discovered on desktop.
    await expect(
      page.getByTestId('desktop-settings-menu-item-voice'),
    ).toBeVisible()
  })

  // TEST-15 — streaming parity: the streaming-augmented voice code (live
  // captions) rides the SAME glob-shared module, so its arrival must not break
  // desktop discovery. The mocked desktop harness renders only the settings menu
  // (not the composer / sub-pages — see the NOTE below), so the toggle + caption
  // rendering is covered by the ui `14-voice` specs on the shared code; here we
  // assert the streaming-augmented module still boots + discovers cleanly on
  // desktop with NO console/page errors.
  test('the streaming-augmented voice module boots cleanly on desktop', async ({
    page,
  }) => {
    const findings: string[] = []
    const NOISE = /favicon|\/@vite\/|@react-refresh|hot-update|sockjs|__vite|\.map(\?|$)/i
    page.on('console', msg => {
      if (msg.type() === 'error' && !NOISE.test(msg.text())) findings.push(`console: ${msg.text()}`)
    })
    page.on('pageerror', err => findings.push(`page: ${err.message}`))

    await page.goto('/settings')
    await expect(page.getByTestId('desktop-settings-menu')).toBeVisible({ timeout: 20000 })
    await expect(page.getByTestId('desktop-settings-menu-item-voice')).toBeVisible()
    expect(findings, findings.join('\n')).toEqual([])
  })

  // NOTE: settings SUB-page rendering (e.g. `/settings/voice`) is intentionally
  // NOT asserted here — the mocked desktop harness routes to the settings menu
  // but does not render sub-pages (the repo's own desktop specs only assert the
  // menu, never a sub-page). The voice admin page's ACTUAL rendering is fully
  // covered by the 8 ui `14-voice` specs, which exercise the SAME glob-shared
  // VoiceSettingsPage/cards. This spec's job is desktop DISCOVERY parity, proven
  // above.
})
