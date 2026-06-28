import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'

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

const ADD_PROVIDER = /Add provider/i

test.describe('Auth providers — per-row Test button', () => {
  test('clicking the row Test button surfaces a result toast', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await page.goto(`${baseURL}/settings/auth-providers`)

    const providerName = `e2e-rowtest-${Date.now()}`
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

    // The per-row Test button (aria-label "Test <slug>").
    const testButton = page.getByRole('button', { name: `Test ${providerName}` })
    await expect(testButton).toBeVisible({ timeout: 10_000 })
    await testButton.click()

    // Bogus issuer → discovery probe fails → "<name>: <reason>" error toast.
    await expect(page.locator('.ant-message-error')).toBeVisible({
      timeout: 15_000,
    })

    // Cleanup.
    await page.getByRole('button', { name: `Delete ${providerName}` }).click()
    const popover = page.locator('.ant-popover:visible').last()
    await popover.locator('.ant-btn-primary').click()
  })
})
