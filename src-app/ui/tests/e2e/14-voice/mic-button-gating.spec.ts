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
 * TEST-27 — Composer mic-button gating.
 *
 *  - ready capability → mic enabled, with aria-label + aria-pressed.
 *  - enabled-but-not-ready → mic present but disabled (aria-disabled) with a
 *    remediation tooltip/description.
 *  - feature off (capability.enabled=false) → mic hidden.
 *  - getUserMedia denied → an error toast surfaces.
 */
test.describe('Voice — mic-button gating (TEST-27)', () => {
  test('ready → mic enabled with aria-label + aria-pressed', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await installVoiceBrowserMocks(page)
    await routeVoice(page, defaultVoiceState())

    await loginAsAdmin(page, baseURL)
    await gotoComposer(page, baseURL)

    const mic = byTestId(page, 'voice-mic-button').first()
    await expect(mic).toBeVisible()
    await expect(mic).toHaveAttribute('aria-label', 'Start voice dictation')
    await expect(mic).toHaveAttribute('aria-pressed', 'false')
    // A ready mic is NOT aria-disabled.
    await expect(mic).not.toHaveAttribute('aria-disabled', 'true')
  })

  test('enabled-but-not-ready → mic disabled with remediation', async ({
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
    await expect(mic).toHaveAttribute(
      'aria-label',
      'Voice dictation (unavailable)',
    )
    // The remediation description is present + reachable via aria-describedby.
    await expect(page.locator('#voice-mic-not-ready-help')).toContainText(
      /contact an administrator/i,
    )
  })

  test('feature off → mic hidden', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await installVoiceBrowserMocks(page)
    await routeVoice(
      page,
      defaultVoiceState({
        capability: readyCapability({ enabled: false, can_transcribe: false }),
      }),
    )

    await loginAsAdmin(page, baseURL)
    await gotoComposer(page, baseURL)

    // Composer is up, but the mic never renders.
    await expect(byTestId(page, 'chat-message-textarea')).toBeVisible()
    await expect(byTestId(page, 'voice-mic-button')).toHaveCount(0)
  })

  test('getUserMedia denied → error toast', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await installVoiceBrowserMocks(page, { denyPermission: true })
    await routeVoice(page, defaultVoiceState())

    await loginAsAdmin(page, baseURL)
    await gotoComposer(page, baseURL)

    const hintDismiss = byTestId(page, 'voice-privacy-hint-dismiss')
    if (await hintDismiss.isVisible().catch(() => false)) {
      await hintDismiss.click()
    }

    await byTestId(page, 'voice-mic-button').first().click()

    await expect(
      page.locator('[data-sonner-toast][data-type="error"]'),
    ).toContainText(/microphone access was denied/i, { timeout: 10000 })
  })
})
