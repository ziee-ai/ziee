import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'

/**
 * E2E — the per-row enable/disable Switch on the Auth Providers list
 * (AuthProvidersListSection.tsx `onToggle`).
 *
 * Audit gap: auth-providers-crud.spec.ts only asserts the Switch renders
 * and starts OFF; clicking it (the enable transition + its health-check
 * snap-back) was never exercised. Enabling a Generic-OIDC provider with a
 * bogus issuer fails the backend enable-transition probe, so the store
 * emits `auto_disabled`, the Switch snaps back to unchecked, and an error
 * toast surfaces — a fully real path (no mocks).
 */

const ADD_PROVIDER = /Add provider/i

test.describe('Auth providers — enable toggle', () => {
  test('toggling enable on a bogus OIDC provider snaps the Switch back', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await page.goto(`${baseURL}/settings/auth-providers`)

    const providerName = `e2e-toggle-${Date.now()}`
    await page.getByRole('button', { name: ADD_PROVIDER }).click()
    await page.getByRole('menuitem', { name: /Generic OIDC/i }).click()
    await expect(
      page.getByRole('button', { name: /^Create$/ }),
    ).toBeVisible({ timeout: 10_000 })
    await page.getByLabel(/Name \(URL slug\)/i).fill(providerName)
    await page.getByLabel(/Client ID/i).fill('e2e-client-id')
    await page.locator('input[type="password"]').first().fill('e2e-secret')
    await page.getByLabel(/Issuer URL/i).fill('https://nonexistent.invalid/oidc')
    await page.getByRole('button', { name: /^Create$/ }).click()

    const toggle = page.getByRole('switch', { name: `Toggle ${providerName}` })
    await expect(toggle).toBeVisible({ timeout: 10_000 })
    await expect(toggle).not.toBeChecked()

    // Attempt to enable → backend probe fails → error toast + snap-back.
    await toggle.click()
    await expect(page.locator('.ant-message-error')).toBeVisible({
      timeout: 15_000,
    })
    await expect(toggle).not.toBeChecked()

    // Cleanup.
    await page.getByRole('button', { name: `Delete ${providerName}` }).click()
    const popover = page.locator('.ant-popover:visible').last()
    await popover.locator('.ant-btn-primary').click()
  })
})
