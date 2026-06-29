import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { byTestId } from '../testid.ts'

/**
 * E2E — auth-provider list-page enable/disable Switch INTERACTION
 * (AuthProvidersListSection.tsx). The crud spec only asserts the seeded
 * providers START disabled; it never clicks the toggle. Enabling an
 * UNCONFIGURED provider triggers the backend enable-transition health probe,
 * which fails (AUTH_PROVIDER_ENABLE_FAILED_HEALTH_CHECK) → an error toast + the
 * Switch snaps back to OFF (the store emits auth_provider.auto_disabled).
 */

test.describe('Auth providers — enable/disable toggle', () => {
  test('toggling an unconfigured provider ON fails the probe and reverts the Switch', async ({
    page,
    testInfra,
  }) => {
    await loginAsAdmin(page, testInfra.baseURL)
    await page.goto(`${testInfra.baseURL}/settings/auth-providers`)

    // The migration-47-seeded "google" OIDC provider starts disabled (no config).
    const toggle = byTestId(page, 'authprov-toggle-switch-google')
    await expect(toggle).toBeVisible({ timeout: 30000 })
    await expect(toggle).toHaveAttribute('aria-checked', 'false')

    // Click to ENABLE → the unconfigured-provider health probe fails.
    await toggle.click()

    // An error toast surfaces the probe failure reason…
    await expect(
      page.locator('[data-sonner-toast][data-type="error"]').first(),
    ).toBeVisible({ timeout: 15000 })
    // …and the Switch snaps back to OFF (auto-disabled — never silently enabled).
    await expect(toggle).toHaveAttribute('aria-checked', 'false', {
      timeout: 15000,
    })
  })
})
