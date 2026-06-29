import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'
import {
  loginAsAdmin,
  getAdminToken,
  createTestUser,
} from '../../common/auth-helpers'
import { navigateToUsers, openUserGroupsDrawer } from './helpers/user-navigation'
import {
  assignUserToGroupInDrawer,
  removeUserFromGroup,
} from './helpers/group-actions'

/**
 * E2E — the user↔group assign + remove workflow through the drawers. Setup
 * (user + a fresh group) is via the admin API; the assignment + removal run
 * through the real UI.
 */

async function apiSetup(apiURL: string) {
  const token = await getAdminToken(apiURL)
  const stamp = Date.now().toString(36)
  const username = `grpflow_${stamp}`
  await createTestUser(
    apiURL,
    token,
    username,
    `${username}@ex.com`,
    'password123',
    ['profile::read', 'profile::edit'],
  )
  const groupName = `E2E Flow Group ${stamp}`
  const res = await fetch(`${apiURL}/api/groups`, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      Authorization: `Bearer ${token}`,
    },
    body: JSON.stringify({
      name: groupName,
      description: 'assign/remove e2e',
      permissions: [],
    }),
  })
  if (!res.ok)
    throw new Error(`create group failed: ${res.status} ${await res.text()}`)
  return { username, groupName }
}

test.describe('Users — group assign + remove workflow', () => {
  test('AssignGroupDrawer assigns a user to a group', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    const { username, groupName } = await apiSetup(apiURL)

    await loginAsAdmin(page, baseURL)
    await navigateToUsers(page, baseURL)
    await openUserGroupsDrawer(page, username)

    // Open the AssignGroupDrawer (multi-group checkboxes) and assign.
    await byTestId(page, 'user-groups-drawer-assign-button').click()
    await byTestId(page, 'user-assign-group-form').waitFor({ state: 'visible' })
    await byTestId(page, 'user-assign-group-checkboxes').waitFor({ state: 'visible' })
    await page
      .locator('[data-testid^="user-assign-group-checkboxes-opt-"]')
      .filter({ hasText: groupName })
      .click()
    await byTestId(page, 'user-assign-group-submit-button').click()
    await expect(
      page.locator('[data-sonner-toast][data-type="success"]').first(),
    ).toBeVisible({ timeout: 10000 })

    // Reopen the groups drawer: the group now carries the Member tag.
    await openUserGroupsDrawer(page, username)
    await expect(
      byTestId(page, `user-groups-drawer-row-${groupName}`).locator(
        '[data-testid^="user-groups-drawer-member-tag-"]',
      ),
    ).toBeVisible({ timeout: 10000 })
  })

  test('UserGroupsDrawer removes a user from a group via the Confirm', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    const { username, groupName } = await apiSetup(apiURL)

    await loginAsAdmin(page, baseURL)
    await navigateToUsers(page, baseURL)
    await openUserGroupsDrawer(page, username)

    // Assign via the per-row control, then remove + confirm.
    await assignUserToGroupInDrawer(page, groupName)
    await removeUserFromGroup(page, groupName)

    // The inline Assign action is back.
    await expect(
      byTestId(page, `user-groups-drawer-row-${groupName}`).locator(
        '[data-testid^="user-groups-drawer-assign-row-button-"]',
      ),
    ).toBeVisible()
  })
})
