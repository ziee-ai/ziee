import { Permissions } from '../../../src/api-client/permissions'
import { loginWithPerms } from '../permissions/fixtures'
import { expect, test } from '../permissions/no-403'
import { byTestId } from '../testid'
import {
  defaultVoiceState,
  installVoiceBrowserMocks,
  mkVoiceModel,
  routeVoice,
} from './voice-helpers'

/**
 * TEST-24 [negative-perm] — model-management authorization on /settings/voice.
 *
 *  - A user with ONLY `voice::admin::read` sees the page and the model lists but
 *    NO manage controls (Install / Upload / Set-active / Delete), and drives no
 *    unexpected 403 (the no-403 fixture) — the store reads self-gate correctly.
 *  - A user without `voice::admin::read` cannot reach the page at all.
 */
test.describe('Voice model management — read-only user (TEST-24)', () => {
  test('read-only voice admin sees the lists but no manage controls', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await installVoiceBrowserMocks(page)
    await routeVoice(
      page,
      defaultVoiceState({
        // One active + one inactive installed model so the (absent) Set-active
        // and Delete controls have rows to hang off.
        models: [
          mkVoiceModel('base', { is_active: true }),
          mkVoiceModel('small'),
        ],
      }),
    )

    await loginWithPerms(
      page,
      baseURL,
      apiURL,
      [Permissions.VoiceAdminRead],
      'voice-ro',
    )
    await page.goto(`${baseURL}/settings/voice`)
    await expect(byTestId(page, 'voice-settings-page-title')).toBeVisible({
      timeout: 30000,
    })

    // The read surfaces are present.
    await expect(byTestId(page, 'voice-available-models-card')).toBeVisible()
    await expect(byTestId(page, 'voice-installed-models-card')).toBeVisible()
    await expect(
      byTestId(page, 'voice-installed-model-row-small'),
    ).toBeVisible()

    // No manage affordances anywhere.
    await expect(byTestId(page, 'voice-model-upload-open-btn')).toHaveCount(0)
    await expect(
      byTestId(page, 'voice-available-model-install-base'),
    ).toHaveCount(0)
    await expect(byTestId(page, 'voice-model-add-url-form')).toHaveCount(0)
    await expect(
      byTestId(page, 'voice-installed-model-activate-small'),
    ).toHaveCount(0)
    await expect(
      byTestId(page, 'voice-installed-model-delete-small'),
    ).toHaveCount(0)
    await expect(
      byTestId(page, 'voice-installed-model-delete-base'),
    ).toHaveCount(0)

    // The config card renders its read-only banner (no Save/manage).
    await expect(byTestId(page, 'voice-config-readonly-alert')).toBeVisible()
  })
})

test.describe('Voice settings — no read permission (TEST-24 negative)', () => {
  // This test intentionally provokes the route/section 403 gate.
  test.use({ allow403: true })

  test('a user without voice::admin::read cannot reach the page', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginWithPerms(page, baseURL, apiURL, [], 'voice-noperm')
    await page.goto(`${baseURL}/settings/voice`)

    // A 403 gate renders in place of the page (router- or settings-section-level).
    await expect(
      page.locator(
        '[data-testid="router-route-forbidden-result"], [data-testid="settings-forbidden-result"]',
      ),
    ).toBeVisible({ timeout: 15000 })
    await expect(byTestId(page, 'voice-settings-page-title')).toHaveCount(0)
  })
})
