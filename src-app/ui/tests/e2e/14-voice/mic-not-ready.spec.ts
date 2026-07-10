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
 * TEST-34 — Not-ready + unsupported postures + the one-time privacy hint.
 *
 *  - enabled-but-not-ready → disabled mic whose remediation says "contact an
 *    administrator".
 *  - unsupported browser (no getUserMedia/MediaRecorder) → mic hidden even when
 *    the capability is fully ready.
 *  - the privacy hint shows once, and stays dismissed across a reload.
 */
test.describe('Voice — not-ready / unsupported / privacy hint (TEST-34)', () => {
  test('not-ready capability → disabled mic points at an administrator', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await installVoiceBrowserMocks(page)
    await routeVoice(
      page,
      defaultVoiceState({
        capability: readyCapability({
          runtime_ready: false,
          model_ready: false,
          can_transcribe: false,
        }),
      }),
    )

    await loginAsAdmin(page, baseURL)
    await gotoComposer(page, baseURL)

    const mic = byTestId(page, 'voice-mic-button').first()
    await expect(mic).toBeVisible()
    await expect(mic).toHaveAttribute('aria-disabled', 'true')
    await expect(page.locator('#voice-mic-not-ready-help')).toContainText(
      /contact an administrator/i,
    )
  })

  test('unsupported browser (no capture APIs) → mic hidden', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    // recording:false strips getUserMedia + MediaRecorder → isRecordingSupported() false.
    await installVoiceBrowserMocks(page, { recording: false })
    await routeVoice(page, defaultVoiceState()) // capability fully ready

    await loginAsAdmin(page, baseURL)
    await gotoComposer(page, baseURL)

    await expect(byTestId(page, 'chat-message-textarea')).toBeVisible()
    await expect(byTestId(page, 'voice-mic-button')).toHaveCount(0)
  })

  test('privacy hint shows once, stays dismissed after reload', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await installVoiceBrowserMocks(page)
    await routeVoice(page, defaultVoiceState())

    await loginAsAdmin(page, baseURL)
    await gotoComposer(page, baseURL)

    // First visit: the one-time hint is shown.
    const hint = byTestId(page, 'voice-privacy-hint')
    await expect(hint).toBeVisible({ timeout: 10000 })
    await expect(hint).toContainText(/transcribed locally/i)

    await byTestId(page, 'voice-privacy-hint-dismiss').click()
    await expect(hint).toHaveCount(0)

    // Reload: the hint stays dismissed (localStorage flag persisted).
    await page.reload({ waitUntil: 'load' })
    await page.waitForSelector('[data-testid="chat-message-textarea"]', {
      timeout: 30000,
    })
    // Mic still present + ready...
    await expect(byTestId(page, 'voice-mic-button').first()).toBeVisible()
    // ...but the hint does not reappear.
    await expect(byTestId(page, 'voice-privacy-hint')).toHaveCount(0)
  })
})
