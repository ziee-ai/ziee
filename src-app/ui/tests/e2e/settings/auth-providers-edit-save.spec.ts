import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import { byTestId } from '../testid.ts'

/**
 * E2E — AuthProviderEditDrawer primary SAVE success path (the disabled-save
 * branch → "Saved <name>"). The crud spec opens the edit drawer but only
 * Cancels; it never asserts a successful save. (The enable-save-with-probe
 * success needs a real OIDC issuer, so this covers the config-edit save that
 * admins use day-to-day.)
 */

test.describe('Auth providers — edit + save', () => {
  test('editing a provider config and saving shows the success toast', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)

    // Seed a Generic OIDC provider (disabled, empty config) to edit.
    const name = `e2e-edit-save-${Date.now()}`
    const res = await fetch(`${apiURL}/api/admin/auth-providers`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        Authorization: `Bearer ${token}`,
      },
      body: JSON.stringify({ name, provider_type: 'oidc', config: {} }),
    })
    expect(res.ok).toBeTruthy()

    await page.goto(`${baseURL}/settings/auth-providers`)
    await byTestId(page, `authprov-edit-button-${name}`).click()

    // The name (URL slug) is immutable in edit mode; change a config field.
    await expect(byTestId(page, 'authprov-name-input')).toBeDisabled({
      timeout: 10000,
    })
    await byTestId(page, 'authprov-oidc-client-id-input').fill('edited-client-id')

    // Save (disabled-save path — no enable probe) → "Saved <name>".
    await byTestId(page, 'authprov-drawer-save-button').click()
    // `name` is dynamic data this test created → asserting it in the toast is
    // legitimate (not chrome).
    await expect(page.getByText(`Saved ${name}`)).toBeVisible({ timeout: 10000 })
  })
})
