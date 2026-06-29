import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'
import { loginAsAdmin } from '../../common/auth-helpers'

/**
 * E2E — SetupPage redirects away once setup is already done.
 *
 * `SetupPage`'s effect (`SetupPage.tsx:13-27`) navigates to `/` when
 * `needsSetup === false`. This guards both the cross-tab case (tab A still on
 * /setup after tab B finishes setup) and the direct-nav case (hitting /setup
 * once an admin already exists). This drives the deterministic direct-nav arm:
 * after an admin exists, visiting /setup must not stay on the setup form.
 */

test.describe('Setup — redirect when already done', () => {
  test('navigating to /setup after an admin exists redirects away', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra

    // Creates the admin + logs in → needsSetup becomes false.
    await loginAsAdmin(page, baseURL)

    // Direct-nav to /setup → the effect redirects to "/" (off the setup form).
    await page.goto(`${baseURL}/setup`)
    await expect(page).not.toHaveURL(/\/setup/, { timeout: 15000 })

    // The setup-only "Confirm Password" field is not present after redirect.
    await expect(byTestId(page, 'app-setup-confirm-password-input')).toHaveCount(0)
  })
})
