import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { byTestId } from '../testid'
import {
  installVoiceBrowserMocks,
  routeVoice,
  defaultVoiceState,
} from './voice-helpers'

/**
 * TEST-29 — Voice settings admin: edit config (idle / caps / language),
 * download the model, save, and confirm everything persists across a reload
 * (GET/PUT /api/voice/settings + POST /api/voice/model/download round-trips).
 */
test.describe('Voice — settings admin (TEST-29)', () => {
  test('edit config + download model, save, and persist across reload', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await installVoiceBrowserMocks(page)
    await routeVoice(
      page,
      // Start with the model NOT present so the download flips its badge.
      defaultVoiceState({
        modelStatus: { model: 'base', present: false },
      }),
    )

    await loginAsAdmin(page, baseURL)
    await page.goto(`${baseURL}/settings/voice`)
    await expect(byTestId(page, 'voice-settings-page-title')).toBeVisible({
      timeout: 30000,
    })

    // ── Download the model: missing → present. ──
    const modelCard = byTestId(page, 'voice-model-card')
    await expect(byTestId(modelCard, 'voice-model-missing-tag')).toBeVisible()
    await byTestId(page, 'voice-model-download-btn').click()
    await expect(byTestId(modelCard, 'voice-model-present-tag')).toBeVisible({
      timeout: 10000,
    })

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
    // The model stays present after reload.
    await expect(
      byTestId(byTestId(page, 'voice-model-card'), 'voice-model-present-tag'),
    ).toBeVisible()
  })
})
