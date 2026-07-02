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
 * The user-profile widget dropdown's "Profile" item is gated on
 * `profile::read` (UserProfileWidget.tsx canViewProfile). A user WITHOUT that
 * permission still bootstraps (GET /auth/me is JwtAuth-only) and sees the
 * widget, but the dropdown must offer only "Logout" — no "Profile". The
 * positive case (admin sees Profile + navigates) is covered by
 * settings/profile.spec's "opens from the user-profile widget dropdown".
 */
test.describe('Profile - widget dropdown permission gating', () => {
  test('a user without profile::read sees Logout but not Profile in the widget', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)

    // profile::edit (to finish onboarding) but deliberately NO profile::read.
    const username = `noprofread_${Date.now().toString(36)}`
    const userId = await createTestUser(
      apiURL,
      adminToken,
      username,
      `${username}@ex.com`,
      'password123',
      ['profile::edit'],
    )

    // create_user unconditionally assigns every new user to the default "Users"
    // group, which grants profile::read (migration 27). So a direct-perms-only
    // user still INHERITS profile::read via the group — the negative path is
    // unreachable unless we strip that group. Remove the user from the default
    // group so they genuinely lack profile::read (keeping only their direct
    // profile::edit), which is exactly the state the widget gate must handle.
    const groupsRes = await fetch(`${apiURL}/api/groups`, {
      headers: { Authorization: `Bearer ${adminToken}` },
    })
    const groupsBody = await groupsRes.json()
    const groups: Array<{ id: string; name: string; is_default: boolean }> =
      groupsBody.groups ?? groupsBody.data ?? groupsBody
    const defaultGroup = groups.find(g => g.is_default) ?? groups.find(g => g.name === 'Users')
    if (!defaultGroup) throw new Error('default Users group not found')
    const removeRes = await fetch(
      `${apiURL}/api/groups/${userId}/${defaultGroup.id}/remove`,
      { method: 'DELETE', headers: { Authorization: `Bearer ${adminToken}` } },
    )
    if (!removeRes.ok) {
      throw new Error(`failed to remove user from default group: ${removeRes.status}`)
    }

    await login(page, baseURL, username, 'password123')

    // Mark onboarding complete (ProfileEdit-gated) so the app shell — and the
    // sidebar widget — render instead of the onboarding wizard.
    const token = await getCurrentUserToken(page)
    await completeOnboarding(baseURL, token)
    await page.goto(`${baseURL}/`)

    const widget = byTestId(page, 'user-profile-widget')
    await expect(widget).toBeVisible({ timeout: 30000 })
    await widget.click()

    // Logout is always offered; Profile is gated out for this user.
    await expect(
      byTestId(page, 'userprofile-menu-dropdown-item-logout'),
    ).toBeVisible()
    await expect(
      byTestId(page, 'userprofile-menu-dropdown-item-profile'),
    ).toHaveCount(0)
  })
})
