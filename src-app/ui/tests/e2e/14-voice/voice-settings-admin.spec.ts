import { loginAsAdmin } from '../../common/auth-helpers'
import { expect, test } from '../../fixtures/test-context'
import { byTestId } from '../testid'
import {
  defaultVoiceState,
  installVoiceBrowserMocks,
  mkVoiceModel,
  routeVoice,
} from './voice-helpers'

/**
 * TEST-29 — Voice settings admin: edit config (idle / caps / language),
 * download the model, save, and confirm everything persists across a reload
 * (GET/PUT /api/voice/settings + POST /api/voice/model/download round-trips).
 */
test.describe('Voice — settings admin (TEST-29)', () => {
  test('edit config, save, and persist across reload', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await installVoiceBrowserMocks(page)
    await routeVoice(page, defaultVoiceState())

    await loginAsAdmin(page, baseURL)
    await page.goto(`${baseURL}/settings/voice`)
    await expect(byTestId(page, 'voice-settings-page-title')).toBeVisible({
      timeout: 30000,
    })

    // (Model download/install is exercised by voice-model-mgmt.spec.ts
    // TEST-17/18 against the new Available/Installed cards; the old single
    // ModelCard was removed in the model-library rework.)

    // ── Edit the config fields. ──
    const configCard = byTestId(page, 'voice-config-card')
    await expect(configCard).toBeVisible()

    const idle = byTestId(page, 'voice-config-idle-unload')
    await idle.click()
    await idle.fill('600')

    const maxClip = byTestId(page, 'voice-config-max-clip')
    await maxClip.click()
    await maxClip.fill('45')

    // Language Select: auto → English.
    await byTestId(page, 'voice-config-language').click()
    await byTestId(page, 'voice-config-language-opt-en').click()

    // ── Save. ──
    await byTestId(page, 'voice-config-save-btn').click()
    await expect(page.locator('[data-sonner-toast]').first()).toBeVisible({
      timeout: 10000,
    })

    // ── Reload → everything persisted. ──
    await page.reload({ waitUntil: 'load' })
    await expect(byTestId(page, 'voice-settings-page-title')).toBeVisible({
      timeout: 30000,
    })
    await expect(byTestId(page, 'voice-config-idle-unload')).toHaveValue('600')
    await expect(byTestId(page, 'voice-config-max-clip')).toHaveValue('45')
    await expect(byTestId(page, 'voice-config-language')).toContainText(
      'English',
    )
  })
})

/**
 * TEST-21 — the reworked model surface: the Available + Installed model cards
 * replace the old single ModelCard, and the not-ready banner reflects whether
 * an installed model covers the configured `settings.model` pointer.
 */
test.describe('Voice — reworked model cards (TEST-21)', () => {
  test('Available + Installed cards replace ModelCard; banner shows when no matching model is installed', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await installVoiceBrowserMocks(page)
    // enabled + a runtime installed, but NO installed models → not ready.
    await routeVoice(page, defaultVoiceState({ models: [] }))

    await loginAsAdmin(page, baseURL)
    await page.goto(`${baseURL}/settings/voice`)
    await expect(byTestId(page, 'voice-settings-page-title')).toBeVisible({
      timeout: 30000,
    })

    // Both new cards render; the old single ModelCard is gone.
    await expect(byTestId(page, 'voice-available-models-card')).toBeVisible()
    await expect(byTestId(page, 'voice-installed-models-card')).toBeVisible()
    await expect(byTestId(page, 'voice-model-card')).toHaveCount(0)

    // No installed model covers settings.model ('base') → not-ready banner shows,
    // and the installed card is empty.
    await expect(byTestId(page, 'voice-not-ready-banner')).toBeVisible()
    await expect(byTestId(page, 'voice-installed-models-empty')).toBeVisible()
  })

  test('not-ready banner clears once a matching model is installed', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await installVoiceBrowserMocks(page)
    // A `base` model is installed, matching settings.model → ready, no banner.
    await routeVoice(
      page,
      defaultVoiceState({
        models: [mkVoiceModel('base', { is_active: true })],
      }),
    )

    await loginAsAdmin(page, baseURL)
    await page.goto(`${baseURL}/settings/voice`)
    await expect(byTestId(page, 'voice-settings-page-title')).toBeVisible({
      timeout: 30000,
    })

    await expect(byTestId(page, 'voice-installed-model-row-base')).toBeVisible()
    await expect(byTestId(page, 'voice-not-ready-banner')).toHaveCount(0)
  })
})
