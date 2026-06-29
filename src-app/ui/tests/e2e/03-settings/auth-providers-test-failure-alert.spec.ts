import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import { byTestId } from '../testid.ts'

/**
 * E2E — the row-level `last_test_ok === false` error Alert
 * (AuthProvidersListSection.tsx). The crud spec covers the DRAWER's inline
 * "Test config" result; the persisted row Alert that renders after the row's
 * "Test <name>" action fails was never asserted. Real backend, no mocks: an
 * empty-config OIDC provider fails discovery deterministically, the server
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
    await byTestId(page, `authprov-test-button-${name}`).click()

    // The row reloads with last_test_ok=false → the persisted error Alert renders.
    await expect(
      byTestId(page, `authprov-test-failed-alert-${name}`),
    ).toBeVisible({ timeout: 20000 })
  })
})
