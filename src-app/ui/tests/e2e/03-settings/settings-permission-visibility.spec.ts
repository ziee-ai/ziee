import { test, expect } from '../../fixtures/test-context'
import {
  loginAsAdmin,
  getAdminToken,
  createTestUser,
  login,
  clearAuthState,
} from '../../common/auth-helpers'
import { byTestId } from '../testid.ts'

/**
 * E2E — the settings menu permission filter (SettingsPage.tsx `isAllowed`).
 *
 * Audit gap: the slot-level permission filter that hides forbidden settings
 * sections was never tested. The admin-only "Hardware" section
 * (settingsAdminPages, gated on hardware::read) is visible to the admin but
 * absent for a regular user who lacks the permission.
 */

test.describe('Settings — permission-filtered menu', () => {
  test('admin sees the Hardware section; a non-permitted user does not', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra

    // Admin: the Hardware admin section is present in the settings menu.
    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)
    await page.goto(`${baseURL}/settings/profile`)
    await expect(
      byTestId(page, 'settings-nav-menu-item-hardware'),
    ).toBeVisible({ timeout: 30000 })

    // Regular user without hardware::read: the section is filtered out.
    const uname = `novis_${Date.now().toString(36)}`
    await createTestUser(
      apiURL,
      adminToken,
      uname,
      `${uname}@example.com`,
      'password123',
      ['profile::read', 'profile::edit'],
    )
    await clearAuthState(page)
    await login(page, baseURL, uname, 'password123')
    await page.goto(`${baseURL}/settings/profile`)
    await expect(byTestId(page, 'settings-page-title')).toBeVisible({
      timeout: 30000,
    })

    // The Hardware menu item is NOT rendered for this user.
    await expect(
      byTestId(page, 'settings-nav-menu-item-hardware'),
    ).toHaveCount(0)
  })
})
