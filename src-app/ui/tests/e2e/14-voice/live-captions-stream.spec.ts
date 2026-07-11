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
 * TEST-10 — live captions stream while recording, and the final authoritative
 * transcript (not the interim) lands in the composer, never auto-sent.
 *
 * With streaming available + the per-device pref ON (the default), recording
 * fires the interim loop against `/api/voice/transcribe/stream`; the returned
 * full transcript renders in the transient live-caption strip. On Stop, the
 * `/api/voice/transcribe` text is appended to the composer, the caption clears,
 * and nothing is sent.
 */
test.describe('Voice — live captions stream (TEST-10)', () => {
  test('interim caption updates while recording; final transcript inserted, not sent', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await installVoiceBrowserMocks(page)
    const voice = await routeVoice(
      page,
      defaultVoiceState({
        streamTranscribe: { text: 'live interim words so far', language: 'en', duration_ms: 90 },
        transcribe: { text: 'the final authoritative sentence', language: 'en', duration_ms: 700 },
      }),
    )

    await loginAsAdmin(page, baseURL)
    await gotoComposer(page, baseURL)

    const textarea = byTestId(page, 'chat-message-textarea')
    await expect(textarea).toHaveValue('')

    // Live captions default ON (streaming_enabled) → the toggle shows pressed.
    await expect(byTestId(page, 'voice-live-toggle')).toHaveAttribute('aria-pressed', 'true')

    const hintDismiss = byTestId(page, 'voice-privacy-hint-dismiss')
    if (await hintDismiss.isVisible().catch(() => false)) await hintDismiss.click()

    await byTestId(page, 'voice-mic-button').first().click()
    await expect(byTestId(page, 'voice-elapsed')).toBeVisible({ timeout: 10000 })

    // The interim loop fires and paints the live caption from the stream endpoint.
    await expect(byTestId(page, 'voice-live-caption')).toContainText('live interim words so far', {
      timeout: 15000,
    })
    expect(voice.state.streamCount).toBeGreaterThanOrEqual(1)

    // Stop → the AUTHORITATIVE final transcript lands in the composer (not the
    // interim text), and the caption is gone.
    await page.getByRole('button', { name: 'Stop recording and transcribe' }).click()
    await expect(textarea).toHaveValue('the final authoritative sentence', { timeout: 15000 })
    await expect(byTestId(page, 'voice-live-caption')).toHaveCount(0)

    // Exactly one FINAL transcribe, and nothing sent.
    expect(voice.state.transcribeCount).toBe(1)
    await expect(byTestId(page, 'chat-message')).toHaveCount(0)
    expect(new URL(page.url()).pathname).not.toMatch(/\/conversations\//)
  })
})
