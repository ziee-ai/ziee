import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { byTestId } from '../testid'
import {
  installVoiceBrowserMocks,
  routeVoice,
  defaultVoiceState,
  gotoComposer,
} from './voice-helpers'

/**
 * TEST-11 — the per-device "Live captions" toggle suppresses / re-enables the
 * interim loop and persists across a reload.
 */
test.describe('Voice — live-captions toggle (TEST-11)', () => {
  // Record, wait long enough that an ENABLED interim loop would have fired
  // (>1 interval), stop, and wait for the FINAL transcript to actually land in
  // the composer — `voice-elapsed` disappears at the 'transcribing' transition,
  // BEFORE the async decode+POST completes, so gating on it alone races the count.
  async function recordBrieflyThenStop(
    page: import('@playwright/test').Page,
    expectedFinal: string,
  ) {
    const hintDismiss = byTestId(page, 'voice-privacy-hint-dismiss')
    if (await hintDismiss.isVisible().catch(() => false)) await hintDismiss.click()
    await byTestId(page, 'voice-mic-button').first().click()
    await expect(byTestId(page, 'voice-elapsed')).toBeVisible({ timeout: 10000 })
    await page.waitForTimeout(2500)
    await page.getByRole('button', { name: 'Stop recording and transcribe' }).click()
    // The batch POST completed only once its text is in the composer.
    await expect(byTestId(page, 'chat-message-textarea')).toHaveValue(expectedFinal, {
      timeout: 15000,
    })
  }

  test('toggle OFF suppresses the interim loop (batch only)', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await installVoiceBrowserMocks(page)
    const voice = await routeVoice(
      page,
      defaultVoiceState({
        transcribe: { text: 'batch only final', language: 'en', duration_ms: 400 },
      }),
    )

    await loginAsAdmin(page, baseURL)
    await gotoComposer(page, baseURL)

    // Default ON → turn it OFF.
    const toggle = byTestId(page, 'voice-live-toggle')
    await expect(toggle).toHaveAttribute('aria-pressed', 'true')
    await toggle.click()
    await expect(toggle).toHaveAttribute('aria-pressed', 'false')

    await recordBrieflyThenStop(page, 'batch only final')

    // No interim requests were made; the batch final ran exactly once.
    expect(voice.state.streamCount).toBe(0)
    expect(voice.state.transcribeCount).toBe(1)
  })

  test('pref persists across reload; ON re-enables the interim loop', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await installVoiceBrowserMocks(page)
    const voice = await routeVoice(page, defaultVoiceState())

    await loginAsAdmin(page, baseURL)
    await gotoComposer(page, baseURL)

    // Turn OFF, then reload — the per-device pref persists.
    await byTestId(page, 'voice-live-toggle').click()
    await expect(byTestId(page, 'voice-live-toggle')).toHaveAttribute('aria-pressed', 'false')
    await page.reload({ waitUntil: 'load' })
    await expect(byTestId(page, 'chat-message-textarea')).toBeVisible({ timeout: 30000 })
    await expect(byTestId(page, 'voice-live-toggle')).toHaveAttribute('aria-pressed', 'false')

    // Turn ON → the interim loop runs again.
    await byTestId(page, 'voice-live-toggle').click()
    await expect(byTestId(page, 'voice-live-toggle')).toHaveAttribute('aria-pressed', 'true')
    await recordBrieflyThenStop(page, 'hello from the voice engine')
    await expect.poll(() => voice.state.streamCount, { timeout: 10000 }).toBeGreaterThanOrEqual(1)
  })
})
