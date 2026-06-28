import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'

/**
 * E2E — the row-level `last_test_ok === false` error Alert
 * (AuthProvidersListSection.tsx:222-237). The crud spec covers the DRAWER's
 * inline "Test config" result; the persisted row Alert that renders after the
 * row's "Test <name>" action fails was never asserted. Real backend, no mocks:
 * an empty-config OIDC provider fails discovery deterministically, the server
 * persists `last_test_ok=false`, the row reloads and shows the Alert.
 */

test.describe('Auth providers — failed Test surfaces the row error Alert', () => {
  test('a failing Test persists last_test_ok=false and renders the error Alert', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)

    // Seed a Generic OIDC provider with empty config so discovery fails fast.
    const name = `e2e-test-fail-${Date.now()}`
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

    // Click the row's Test action; the backend tries discovery and fails.
    await page.getByRole('button', { name: `Test ${name}` }).click()

    // The row reloads with last_test_ok=false → the error Alert renders.
    const alert = page.locator('.ant-alert-error').filter({
      hasText: 'Connection test failed',
    })
    await expect(alert.first()).toBeVisible({ timeout: 20000 })
  })
})
