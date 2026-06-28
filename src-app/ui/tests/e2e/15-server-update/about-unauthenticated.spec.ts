import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'

// audit id all-0b03df3dc8f5 — accessing /settings/about WITHOUT authentication
// must not expose the page. AuthGuard renders the auth page (login) for an
// unauthenticated request rather than the About content.
test.describe('About page — unauthenticated access', () => {
  test('unauthenticated /settings/about shows login, not the About page', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    // Ensure an admin exists (so AuthGuard goes to login, not /setup), then
    // drop the session.
    await loginAsAdmin(page, baseURL)
    await page.evaluate(() => localStorage.removeItem('auth-storage'))

    await page.goto(`${baseURL}/settings/about`)

    // The login form is shown (AuthGuard → AuthPage); the About content is not.
    await expect(page.getByLabel('Username', { exact: true })).toBeVisible({ timeout: 30000 })
    await expect(page.getByText('Server version and updates')).toHaveCount(0)
  })
})
