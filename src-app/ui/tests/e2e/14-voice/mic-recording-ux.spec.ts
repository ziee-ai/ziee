import { test, expect } from '../../fixtures/test-context'
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
 * TEST-35 — Recording UX: elapsed timer, auto-stop at max_clip_seconds, and the
 * staged "Starting voice engine…" → "Transcribing…" status while the transcribe
 * request is in flight (surfaced with a persistent live region for a11y).
 *
 * We pin max_clip_seconds=1 so the recording auto-stops without a Stop click,
 * and slow the transcribe response so both staged labels are observable.
 */
test.describe('Voice — recording UX (TEST-35)', () => {
  test('elapsed timer, auto-stop, and staged transcribe status', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await installVoiceBrowserMocks(page)
    // max_clip_seconds=3 gives a stable ~3s recording window to assert the timer
    // + "Recording started" announcement WITHOUT racing the auto-stop (which
    // overwrites the shared live region with "Transcribing"); the auto-stop is
    // still exercised (no Stop click) when the 3s cap fires.
    await routeVoice(
      page,
      defaultVoiceState({
        capability: readyCapability({ max_clip_seconds: 3 }),
      }),
    )
    // A slow transcribe so the "Starting…" → "Transcribing…" staging is visible.
    // Registered AFTER routeVoice so this more-specific handler wins (LIFO).
    await page.route('**/api/voice/transcribe', async route => {
      await new Promise(r => setTimeout(r, 1800))
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({
          text: 'auto stopped and transcribed',
          language: 'en',
          duration_ms: 500,
        }),
      })
    })

    await loginAsAdmin(page, baseURL)
    await gotoComposer(page, baseURL)

    const hintDismiss = byTestId(page, 'voice-privacy-hint-dismiss')
    if (await hintDismiss.isVisible().catch(() => false)) {
      await hintDismiss.click()
    }

    await byTestId(page, 'voice-mic-button').first().click()

    // Recording indicator + elapsed timer (m:ss).
    const elapsed = byTestId(page, 'voice-elapsed')
    await expect(elapsed).toBeVisible({ timeout: 10000 })
    await expect(elapsed).toHaveText(/^\d:\d\d$/)
    // Live region announced the recording start.
    await expect(byTestId(page, 'voice-live-region')).toContainText(
      'Recording started',
    )

    // Auto-stop (max_clip=1s) flips us into the transcribing state WITHOUT a
    // Stop click — the spinner + staged label appear.
    const transcribing = byTestId(page, 'voice-transcribing')
    await expect(transcribing).toBeVisible({ timeout: 10000 })
    await expect(transcribing).toContainText('Starting voice engine…')
    // ...then the staged label advances while the slow POST is still in flight.
    await expect(transcribing).toContainText('Transcribing…', { timeout: 5000 })

    // Finally the transcript lands and the live region confirms it.
    await expect(byTestId(page, 'chat-message-textarea')).toHaveValue(
      'auto stopped and transcribed',
      { timeout: 15000 },
    )
    await expect(byTestId(page, 'voice-live-region')).toContainText(
      'Transcript added',
    )
  })
})
