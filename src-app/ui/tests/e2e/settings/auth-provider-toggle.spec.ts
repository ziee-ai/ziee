import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { byTestId } from '../testid.ts'

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

test.describe('Auth providers — enable toggle', () => {
  test('toggling enable on a bogus OIDC provider snaps the Switch back', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await page.goto(`${baseURL}/settings/auth-providers`)

    const providerName = `e2e-toggle-${Date.now()}`
    await byTestId(page, 'authprov-add-button').click()
    await byTestId(page, 'authprov-add-dropdown-item-oidc-generic').click()
    await expect(byTestId(page, 'authprov-drawer-save-button')).toBeVisible({
      timeout: 10_000,
    })
    await byTestId(page, 'authprov-name-input').fill(providerName)
    await byTestId(page, 'authprov-oidc-client-id-input').fill('e2e-client-id')
    await byTestId(page, 'authprov-oidc-client-secret-input').fill('e2e-secret')
    await byTestId(page, 'authprov-oidc-issuer-url-input').fill(
      'https://nonexistent.invalid/oidc',
    )
    await byTestId(page, 'authprov-drawer-save-button').click()

    const toggle = byTestId(page, `authprov-toggle-switch-${providerName}`)
    await expect(toggle).toBeVisible({ timeout: 10_000 })
    await expect(toggle).not.toBeChecked()

    // Attempt to enable → backend probe fails → error toast + snap-back.
    await toggle.click()
    await expect(
      page.locator('[data-sonner-toast][data-type="error"]'),
    ).toBeVisible({ timeout: 15_000 })
    await expect(toggle).not.toBeChecked()

    // Cleanup.
    await byTestId(page, `authprov-delete-button-${providerName}`).click()
    await byTestId(
      page,
      `authprov-delete-confirm-${providerName}-confirm`,
    ).click()
  })
})
