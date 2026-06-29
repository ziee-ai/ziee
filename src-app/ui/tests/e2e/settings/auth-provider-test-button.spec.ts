import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { byTestId } from '../testid.ts'

/**
 * E2E — the per-row "Test <name>" button on the Auth Providers list
 * (AuthProvidersListSection.tsx `onTest` → testProvider).
 *
 * Audit gap: the existing crud spec only covers the in-DRAWER "Test config"
 * button; the per-ROW Test action (a distinct affordance with its own
 * loading state) was untested. This creates an OIDC provider with a bogus
 * issuer and clicks its row Test button — the backend discovery probe fails,
 * surfacing the `<name>: <reason>` error toast (real onTest path).
 */

test.describe('Auth providers — per-row Test button', () => {
  test('clicking the row Test button surfaces a result toast', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await page.goto(`${baseURL}/settings/auth-providers`)

    const providerName = `e2e-rowtest-${Date.now()}`
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

    // The per-row Test button.
    const testButton = byTestId(page, `authprov-test-button-${providerName}`)
    await expect(testButton).toBeVisible({ timeout: 10_000 })
    await testButton.click()

    // Bogus issuer → discovery probe fails → "<name>: <reason>" error toast.
    await expect(
      page.locator('[data-sonner-toast][data-type="error"]'),
    ).toBeVisible({ timeout: 15_000 })

    // Cleanup.
    await byTestId(page, `authprov-delete-button-${providerName}`).click()
    await byTestId(
      page,
      `authprov-delete-confirm-${providerName}-confirm`,
    ).click()
  })
})
