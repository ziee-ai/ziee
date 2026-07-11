import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { loginWithPerms } from '../permissions/fixtures'
import { byTestId } from '../testid'
import {
  installVoiceBrowserMocks,
  routeVoice,
  defaultVoiceState,
} from './voice-helpers'

/**
 * TEST-28 — Voice runtime admin: /settings/voice check-for-updates → install
 * (progress → complete) → set-default → delete; plus a non-admin is 403'd.
 */
test.describe('Voice — runtime admin (TEST-28)', () => {
  test('admin installs a runtime, sets it default, and deletes another', async ({
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

    // The available-runtimes card auto-checks on mount and lists the upstream
    // release (v1.1.0 not-yet-installed, v1.0.0 installed).
    await expect(byTestId(page, 'voice-available-versions-card')).toBeVisible()
    await expect(byTestId(page, 'voice-version-row-v1.1.0')).toBeVisible({
      timeout: 15000,
    })
    await expect(byTestId(page, 'voice-version-installed-tag-v1.0.0')).toBeVisible()

    // Manual "Check for updates" re-fetches and reports the one new runtime.
    await byTestId(page, 'voice-check-updates-btn').click()
    await expect(page.locator('[data-sonner-toast]')).toContainText(
      /new runtime|up to date/i,
      { timeout: 10000 },
    )

    // Install v1.1.0 → POST download → SSE (connected/progress/complete). On
    // complete the store reloads versions + the update-check, so v1.1.0 flips to
    // "installed" (this asserts the whole progress→complete pipeline ran).
    await byTestId(page, 'voice-version-install-v1.1.0').click()
    await expect(byTestId(page, 'voice-version-installed-tag-v1.1.0')).toBeVisible({
      timeout: 15000,
    })

    // The Installed card now lists v1.1.0 (non-default) → set it as default.
    const setDefaultV11 = byTestId(page, 'voice-version-set-default-v1.1.0')
    await expect(setDefaultV11).toBeVisible({ timeout: 10000 })
    await setDefaultV11.click()
    // v1.1.0 is now default (its set-default button disappears) and v1.0.0's
    // set-default button appears (it's no longer the default).
    await expect(setDefaultV11).toHaveCount(0)
    await expect(
      byTestId(page, 'voice-version-set-default-v1.0.0'),
    ).toBeVisible({ timeout: 10000 })

    // Delete v1.0.0 via the confirm dialog.
    await byTestId(page, 'voice-version-delete-v1.0.0').click()
    await byTestId(page, 'voice-version-delete-confirm-v1.0.0-confirm').click()
    await expect(byTestId(page, 'voice-version-delete-v1.0.0')).toHaveCount(0, {
      timeout: 10000,
    })
    // v1.1.0 remains installed.
    await expect(byTestId(page, 'voice-version-delete-v1.1.0')).toBeVisible()
  })

  test('non-admin is forbidden from /settings/voice', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    // A user with no voice-admin permission.
    await loginWithPerms(page, baseURL, apiURL, [], 'voice-nonadmin')
    await page.goto(`${baseURL}/settings/voice`)

    // A 403 gate renders in place of the page — accept EITHER the router-level
    // (`router-route-forbidden-result`) or the settings-section-level
    // (`settings-forbidden-result`) fallback: `/settings/voice` resolves through
    // the settings shell, which renders the settings-level 403 for an unpermitted
    // section. Mirrors the proven selector in literature/admin-settings.spec.ts.
    await expect(
      page.locator(
        '[data-testid="router-route-forbidden-result"], [data-testid="settings-forbidden-result"]',
      ),
    ).toBeVisible({ timeout: 15000 })
    await expect(byTestId(page, 'voice-settings-page-title')).toHaveCount(0)
  })
})
