import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { loginWithPerms } from '../permissions/fixtures'
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

  test('no voice::transcribe permission → NO mic affordance (independent of a ready capability)', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await installVoiceBrowserMocks(page)
    // Mock a FULLY-READY capability + supported browser, so the ONLY thing that
    // can hide the mic is the PERMISSION gate — proving it is independent of the
    // feature/binary-availability gate (a ready capability would otherwise SHOW
    // the enabled mic, as the first test in this file asserts).
    await routeVoice(page, defaultVoiceState())

    // A real user isolated to profile-only perms: loginWithPerms removes them
    // from the default Users group, so they do NOT hold `voice::transcribe`
    // (migration 134 grants that via the group, not directly).
    await loginWithPerms(page, baseURL, apiURL, [], 'voice-noperm')
    await gotoComposer(page, baseURL)

    // The composer renders (the /chat route is ungated), but the mic affordance
    // is ENTIRELY absent — not the active mic, and not the muted "not set up"
    // state either. An unpermitted user sees no voice affordance at all — including
    // the streaming surfaces (live-captions toggle + live caption strip).
    await expect(byTestId(page, 'chat-message-textarea')).toBeVisible()
    await expect(byTestId(page, 'voice-mic-button')).toHaveCount(0)
    await expect(byTestId(page, 'voice-elapsed')).toHaveCount(0)
    await expect(byTestId(page, 'voice-live-region')).toHaveCount(0)
    await expect(byTestId(page, 'voice-live-toggle')).toHaveCount(0)
    await expect(byTestId(page, 'voice-live-caption')).toHaveCount(0)
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
