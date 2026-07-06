import { test, expect } from '../../fixtures/test-context'
import {
  loginAsAdmin,
  getAdminToken,
  createTestUser,
  login,
  clearAuthState,
} from '../../common/auth-helpers'
import { createGroupViaAPI } from '../../common/provider-helpers'
import { byTestId } from '../testid'

/**
 * E2E — permission inheritance through GROUP MEMBERSHIP.
 *
 * Audit gap: that a user inherits a permission from a group they're a member
 * of (not granted directly) was untested. A user with NO direct hardware
 * permission, added to a group carrying `hardware::read`, must gain access:
 * the Hardware settings section becomes visible + reachable.
 */

test.describe('Users — group permission inheritance', () => {
  test('a user inherits hardware::read via group membership', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)

    const tag = Date.now().toString(36)
    // A group whose ONLY notable grant is hardware::read.
    const groupId = await createGroupViaAPI(
      apiURL,
      adminToken,
      `HW Group ${tag}`,
      'inherits hardware read',
      ['hardware::read'],
    )

    // A user with NO hardware permission of their own.
    const uname = `inherit_${tag}`
    const userId = await createTestUser(
      apiURL,
      adminToken,
      uname,
      `${uname}@example.com`,
      'password123',
      ['profile::read', 'profile::edit'],
    )

    // Add the user to the group → they inherit hardware::read.
    const assignRes = await fetch(`${apiURL}/api/groups/assign`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        Authorization: `Bearer ${adminToken}`,
      },
      body: JSON.stringify({ user_id: userId, group_id: groupId }),
    })
    expect(assignRes.ok, `assign: ${assignRes.status}`).toBeTruthy()

    await clearAuthState(page)
    await login(page, baseURL, uname, 'password123')

    // Inherited perm: the Hardware settings nav item is now visible…
    await page.goto(`${baseURL}/settings/profile`)
    await expect(
      byTestId(page, 'settings-nav-menu-item-hardware'),
    ).toBeVisible({ timeout: 30000 })

    // …and reachable (the route admits the inherited permission).
    await page.goto(`${baseURL}/settings/hardware`)
    await expect(
      byTestId(page, 'hardware-os-card')
        .or(byTestId(page, 'hardware-settings-error')),
    ).toBeVisible({ timeout: 30000 })
  })
})
