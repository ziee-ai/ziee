import { test, expect } from '../../fixtures/test-context'
import { loginWithPerms } from '../permissions/fixtures'
import { Permissions } from '../../../src/api-client/types'

/**
 * E2E — a NON-admin user can reach /settings/general.
 *
 * The route is `requiresAuth: true` with NO permission gate (general user
 * settings), so a plain authenticated user must be able to open it and see the
 * theme form. The existing settings.spec only ever logs in as admin.
 */

test.describe('Settings — general page (non-admin)', () => {
  test('a non-admin user can open /settings/general and see the theme form', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra

    // A user with only profile perms — no admin/settings management perms.
    await loginWithPerms(page, baseURL, apiURL, [
      Permissions.ProfileRead,
      Permissions.ProfileEdit,
    ])

    await page.goto(`${baseURL}/settings/general`)

    // The General settings page renders for a non-admin (theme form present).
    await expect(
      page.locator('#theme-form [aria-label="Theme"]').first(),
    ).toBeVisible({ timeout: 30000 })
  })
})
