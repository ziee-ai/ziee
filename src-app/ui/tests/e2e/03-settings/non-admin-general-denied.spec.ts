import { test, expect } from '../../fixtures/test-context'
import {
  loginAsAdmin,
  getAdminToken,
  createTestUser,
  login,
  clearAuthState,
} from '../../common/auth-helpers'
import { goToSettingsPage, waitForSettingsPageLoad } from './helpers/navigation-helpers'
import { byTestId } from '../testid.ts'

/**
 * E2E — non-admin access to /settings/general (audit gap all-f7a7970e0c82).
 *
 * The audit framed /settings/general as an "admin-gated section" whose
 * non-admin denial path was untested. That premise is FACTUALLY WRONG:
 * the section is registered in `settings-general/module.tsx` under the
 * `settingsUserPages` slot with a route that is `requiresAuth: true`
 * only — there is NO permission gate. It is a per-user settings page
 * (appearance / theme), so every authenticated user, admin or not, may
 * open it.
 *
 * What WAS genuinely untested is the actual behavior: a NON-admin user
 * reaching /settings/general. Existing settings specs all use
 * `loginAsAdmin`. This proves the real contract — a plain authenticated
 * user is NOT denied: the General settings render (heading + Appearance),
 * with an admin positive control. If someone later (incorrectly) bolts an
 * admin-only gate onto this user page, this test fails loudly.
 */

test.describe('Settings — non-admin access to /settings/general', () => {
  test('a non-admin authenticated user can open General settings (ungated user page)', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    const adminToken = await getAdminToken(apiURL)

    const tag = Date.now().toString(36)
    const username = `genuser_${tag}`
    await createTestUser(
      apiURL,
      adminToken,
      username,
      `${username}@example.com`,
      'password123',
      // Deliberately minimal: profile perms only, NO admin/general perm.
      ['profile::read', 'profile::edit'],
    )

    await clearAuthState(page)
    await login(page, baseURL, username, 'password123')

    // Deep-link straight to the section the audit claimed is admin-gated.
    await goToSettingsPage(page, baseURL, 'general')

    // Not denied: the real General settings render for this non-admin user.
    await waitForSettingsPageLoad(page, 'General')
    await expect(byTestId(page, 'settingsgen-appearance-card')).toBeVisible()
    await expect(byTestId(page, 'settingsgen-theme-select')).toBeVisible()

    // And no inline 403/forbidden Result is shown.
    await expect(byTestId(page, 'settings-forbidden-result')).toHaveCount(0)
  })

  test('positive control: an admin also sees General settings', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await goToSettingsPage(page, baseURL, 'general')
    await waitForSettingsPageLoad(page, 'General')
    await expect(byTestId(page, 'settingsgen-appearance-card')).toBeVisible()
  })
})
