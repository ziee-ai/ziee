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
 * TEST-26 — Dictation inserts transcript into the composer, never auto-sends.
 *
 * Ready capability → click mic → record → stop → the transcribe intercept
 * returns canned text → the text lands in the composer textarea AND no message
 * was sent (message list stays empty, we stay on the new-chat page). A separate
 * flow proves Cancel discards without inserting.
 */
test.describe('Voice — dictation inserts, never sends (TEST-26)', () => {
  test('records → stops → transcript appears in composer with no message sent', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await installVoiceBrowserMocks(page)
    const voice = await routeVoice(
      page,
      defaultVoiceState({
        transcribe: {
          text: 'schedule the follow up appointment',
          language: 'en',
          duration_ms: 720,
        },
      }),
    )

    await loginAsAdmin(page, baseURL)
    await gotoComposer(page, baseURL)

    const textarea = byTestId(page, 'chat-message-textarea')
    await expect(textarea).toHaveValue('')

    // Mic is ready.
    const mic = byTestId(page, 'voice-mic-button')
    await expect(mic.first()).toBeVisible()

    // Dismiss the one-time privacy hint if present, then start recording.
    const hintDismiss = byTestId(page, 'voice-privacy-hint-dismiss')
    if (await hintDismiss.isVisible().catch(() => false)) {
      await hintDismiss.click()
    }

    await byTestId(page, 'voice-mic-button').first().click()

    // Recording UI: elapsed timer + a Stop button (still the mic testid, now
    // labelled "Stop recording and transcribe").
    await expect(byTestId(page, 'voice-elapsed')).toBeVisible({ timeout: 10000 })
    const stop = page.getByRole('button', {
      name: 'Stop recording and transcribe',
    })
    await expect(stop).toBeVisible()
    await stop.click()

    // Transcript lands in the composer.
    await expect(textarea).toHaveValue('schedule the follow up appointment', {
      timeout: 15000,
    })

    // Exactly one transcribe call, and NOTHING was sent.
    expect(voice.state.transcribeCount).toBe(1)
    await expect(byTestId(page, 'chat-message')).toHaveCount(0)
    // Still on the new-chat page (a real send would navigate to a conversation).
    expect(new URL(page.url()).pathname).not.toMatch(/\/conversations\//)
  })

  test('cancel discards the recording — no transcribe, composer stays empty', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await installVoiceBrowserMocks(page)
    const voice = await routeVoice(page, defaultVoiceState())

    await loginAsAdmin(page, baseURL)
    await gotoComposer(page, baseURL)

    const textarea = byTestId(page, 'chat-message-textarea')
    const hintDismiss = byTestId(page, 'voice-privacy-hint-dismiss')
    if (await hintDismiss.isVisible().catch(() => false)) {
      await hintDismiss.click()
    }

    await byTestId(page, 'voice-mic-button').first().click()
    await expect(byTestId(page, 'voice-elapsed')).toBeVisible({ timeout: 10000 })

    // Cancel instead of stop.
    await byTestId(page, 'voice-cancel-button').click()

    // Back to an idle mic, empty composer, and zero transcribe calls.
    await expect(byTestId(page, 'voice-elapsed')).toHaveCount(0)
    await expect(textarea).toHaveValue('')
    expect(voice.state.transcribeCount).toBe(0)
  })
})
