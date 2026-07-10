import { test, expect } from '../../fixtures/test-context'
import type { Page } from '@playwright/test'
import { loginAsAdmin } from '../../common/auth-helpers'
import { byTestId } from '../testid'
import {
  installVoiceBrowserMocks,
  routeVoice,
  defaultVoiceState,
  readyCapability,
  gotoComposer,
} from './voice-helpers'

/**
 * TEST-31 — Runtime-health check for the voice surfaces.
 *
 * The gallery cells for voice are deferred, so instead of a blessed pixel
 * baseline we drive the real mic-button + admin-page states and assert they
 * render with NO console errors, NO uncaught page errors, and NO failed network
 * requests (the gating HIGH categories from `scripts/runtime-health.mjs`),
 * filtering the known harness/SSE noise.
 */

interface HealthProbe {
  findings: string[]
}

const NOISE =
  /favicon|\/@vite\/|@react-refresh|hot-update|sockjs|__vite|\.map(\?|$)/i

function attachHealthProbe(page: Page): HealthProbe {
  const findings: string[] = []
  page.on('console', msg => {
    if (msg.type() !== 'error') return
    const text = msg.text()
    if (NOISE.test(text)) return
    findings.push(`console-error: ${text}`)
  })
  page.on('pageerror', err => {
    findings.push(`page-error: ${err.message}`)
  })
  page.on('requestfailed', req => {
    const url = req.url()
    const errText = req.failure()?.errorText ?? ''
    // Long-lived SSE streams (sync + chat) are torn down on every navigation →
    // an expected ERR_ABORTED; not a real failure.
    if (NOISE.test(url)) return
    if (/\/api\/sync\/subscribe|\/api\/chat\/stream/.test(url)) return
    if (req.resourceType() === 'eventsource') return
    if (/aborted/i.test(errText)) return
    findings.push(`request-failed: ${url} (${errText})`)
  })
  return { findings }
}

test.describe('Voice — runtime health of key states (TEST-31)', () => {
  test('composer mic states render cleanly (idle → record → transcribe)', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await installVoiceBrowserMocks(page)
    await routeVoice(
      page,
      defaultVoiceState({
        transcribe: {
          text: 'clean render transcript',
          language: 'en',
          duration_ms: 400,
        },
      }),
    )
    await loginAsAdmin(page, baseURL)

    // Attach AFTER login so the probe only judges the surface under test.
    const probe = attachHealthProbe(page)
    await gotoComposer(page, baseURL)

    // Idle mic renders.
    await expect(byTestId(page, 'voice-mic-button').first()).toBeVisible()
    const hintDismiss = byTestId(page, 'voice-privacy-hint-dismiss')
    if (await hintDismiss.isVisible().catch(() => false)) {
      await hintDismiss.click()
    }

    // Recording state renders.
    await byTestId(page, 'voice-mic-button').first().click()
    await expect(byTestId(page, 'voice-elapsed')).toBeVisible({ timeout: 10000 })

    // Transcribe state → transcript inserted.
    await page
      .getByRole('button', { name: 'Stop recording and transcribe' })
      .click()
    await expect(byTestId(page, 'chat-message-textarea')).toHaveValue(
      'clean render transcript',
      { timeout: 15000 },
    )

    expect(probe.findings, probe.findings.join('\n')).toEqual([])
  })

  test('admin page renders cleanly (provisioned)', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await installVoiceBrowserMocks(page)
    await routeVoice(page, defaultVoiceState())
    await loginAsAdmin(page, baseURL)

    const probe = attachHealthProbe(page)
    await page.goto(`${baseURL}/settings/voice`)
    await expect(byTestId(page, 'voice-settings-page-title')).toBeVisible({
      timeout: 30000,
    })
    // All five cards render.
    for (const id of [
      'voice-installed-versions-card',
      'voice-available-versions-card',
      'voice-model-card',
      'voice-instance-card',
      'voice-config-card',
    ]) {
      await expect(byTestId(page, id)).toBeVisible()
    }
    // Let the auto update-check settle so its request is judged too.
    await expect(byTestId(page, 'voice-version-row-v1.1.0')).toBeVisible({
      timeout: 15000,
    })

    expect(probe.findings, probe.findings.join('\n')).toEqual([])
  })

  test('admin page renders cleanly (unprovisioned empty state)', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    const now = new Date().toISOString()
    await installVoiceBrowserMocks(page)
    await routeVoice(
      page,
      defaultVoiceState({
        capability: readyCapability({
          runtime_ready: false,
          model_ready: false,
          can_transcribe: false,
        }),
        versions: [],
        modelStatus: { model: 'base', present: false },
        instance: {
          status: 'stopped',
          state: 'stopped',
          restart_attempts: 0,
          state_changed_at: now,
        },
      }),
    )
    await loginAsAdmin(page, baseURL)

    const probe = attachHealthProbe(page)
    await page.goto(`${baseURL}/settings/voice`)
    await expect(byTestId(page, 'voice-not-ready-banner')).toBeVisible({
      timeout: 30000,
    })
    await expect(byTestId(page, 'voice-installed-empty')).toBeVisible()

    expect(probe.findings, probe.findings.join('\n')).toEqual([])
  })
})
