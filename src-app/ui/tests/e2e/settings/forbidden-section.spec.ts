import { test, expect } from '../../fixtures/test-context'
import {
  loginAsAdmin,
  getAdminToken,
  createTestUser,
  login,
  getCurrentUserToken,
  completeOnboarding,
} from '../../common/auth-helpers'
import { byTestId } from '../testid.ts'

/**
 * Deep-linking to a settings section that EXISTS (registered slot) but the
 * user's permissions hide renders an inline 403 panel rather than silently
 * redirecting (SettingsPage.tsx forbiddenSection → <Result status="403">). This
 * makes admin-shared links produce a meaningful page.
 */
test.describe('Settings - forbidden section deep-link', () => {
  test('a user without users::read deep-linking /settings/users sees an inline 403', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)

    // A user who can reach the app (profile::edit → can finish onboarding) but
    // lacks users::read (the gate on the "Users" settings section).
    const username = `nousers_${Date.now().toString(36)}`
    await createTestUser(
      apiURL,
      adminToken,
      username,
      `${username}@ex.com`,
      'password123',
      ['profile::read', 'profile::edit'],
    )
    await login(page, baseURL, username, 'password123')
    const token = await getCurrentUserToken(page)
    await completeOnboarding(baseURL, token)

    // Deep-link to the admin-only Users section.
    await page.goto(`${baseURL}/settings/users`)

    // The inline 403 panel renders (not a redirect away from /settings/users).
    const forbidden = byTestId(page, 'router-route-forbidden-result')
    await expect(forbidden).toBeVisible({ timeout: 30000 })
    // Its subtitle names the section the user may not view.
    await expect(forbidden).toContainText(/don't have permission to view/i)
  })
})
