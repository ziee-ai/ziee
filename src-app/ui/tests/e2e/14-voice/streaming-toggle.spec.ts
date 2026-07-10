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
  async function recordBrieflyThenStop(page: import('@playwright/test').Page) {
    const hintDismiss = byTestId(page, 'voice-privacy-hint-dismiss')
    if (await hintDismiss.isVisible().catch(() => false)) await hintDismiss.click()
    await byTestId(page, 'voice-mic-button').first().click()
    await expect(byTestId(page, 'voice-elapsed')).toBeVisible({ timeout: 10000 })
    // Record long enough that an ENABLED interim loop would have fired (>1 interval).
    await page.waitForTimeout(2500)
    await page.getByRole('button', { name: 'Stop recording and transcribe' }).click()
    await expect(byTestId(page, 'voice-elapsed')).toHaveCount(0, { timeout: 15000 })
  }

  test('toggle OFF suppresses the interim loop (batch only)', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await installVoiceBrowserMocks(page)
    const voice = await routeVoice(page, defaultVoiceState())

    await loginAsAdmin(page, baseURL)
    await gotoComposer(page, baseURL)

    // Default ON → turn it OFF.
    const toggle = byTestId(page, 'voice-live-toggle')
    await expect(toggle).toHaveAttribute('aria-pressed', 'true')
    await toggle.click()
    await expect(toggle).toHaveAttribute('aria-pressed', 'false')

    await recordBrieflyThenStop(page)

    // No interim requests were made; the batch final still ran.
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
    await recordBrieflyThenStop(page)
    expect(voice.state.streamCount).toBeGreaterThanOrEqual(1)
  })
})
