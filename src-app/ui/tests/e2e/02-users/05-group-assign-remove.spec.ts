import { test, expect } from '../../fixtures/test-context'
import {
  loginAsAdmin,
  getAdminToken,
  createTestUser,
} from '../../common/auth-helpers'
import { navigateToUsers } from './helpers/user-navigation'
import { openUserGroupsDrawer } from './helpers/user-navigation'

/**
 * E2E — the user↔group assign + remove workflow through the drawers.
 *
 * Prior coverage stopped at OPENING the UserGroupsDrawer (04-group-members
 * 'should display user groups drawer'); the AssignGroupDrawer (the child drawer
 * with the group checkboxes) had ZERO coverage, and the UserGroupsDrawer's own
 * assign/remove actions were never exercised. Setup (user + a fresh group) is
 * via the admin API; the assignment + removal run through the real UI.
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
    headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${token}` },
    body: JSON.stringify({ name: groupName, description: 'assign/remove e2e', permissions: [] }),
  })
  if (!res.ok) throw new Error(`create group failed: ${res.status} ${await res.text()}`)
  return { username, groupName }
}

test.describe('Users — group assign + remove workflow', () => {
  test('AssignGroupDrawer assigns a user to a group', async ({ page, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    const { username, groupName } = await apiSetup(apiURL)

    await loginAsAdmin(page, baseURL)
    await navigateToUsers(page, baseURL)
    await openUserGroupsDrawer(page, username)

    const groupsDrawer = page.locator('.ant-drawer.ant-drawer-open')
    // Open the AssignGroupDrawer (the child drawer with the checkbox list).
    await groupsDrawer.getByRole('button', { name: 'Assign group' }).click()
    const assignDrawer = page.getByRole('dialog', { name: 'Assign to Group' })
    await expect(assignDrawer).toBeVisible({ timeout: 10000 })

    // Check the fresh group + submit.
    await assignDrawer.getByRole('checkbox', { name: groupName }).check()
    await assignDrawer.locator('.ant-btn-primary[type="submit"]').click()
    await expect(page.locator('.ant-message-success').first()).toBeVisible({ timeout: 10000 })

    // Back in the UserGroupsDrawer the group now carries the "Member" tag.
    const memberRow = page
      .locator('.ant-drawer.ant-drawer-open .ant-list-item')
      .filter({ hasText: groupName })
    await expect(memberRow.getByText('Member')).toBeVisible({ timeout: 10000 })
  })

  test('UserGroupsDrawer removes a user from a group via the Popconfirm', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    const { username, groupName } = await apiSetup(apiURL)

    await loginAsAdmin(page, baseURL)
    await navigateToUsers(page, baseURL)
    await openUserGroupsDrawer(page, username)

    // Assign first (inline Assign action on the group row).
    const drawer = page.locator('.ant-drawer.ant-drawer-open')
    const row = drawer.locator('.ant-list-item').filter({ hasText: groupName })
    await row.getByRole('button', { name: 'Assign', exact: true }).click()
    await expect(page.locator('.ant-message-success').first()).toBeVisible({ timeout: 10000 })
    await expect(row.getByText('Member')).toBeVisible({ timeout: 10000 })

    // Now Remove → Popconfirm → confirm.
    await row.getByRole('button', { name: 'Remove', exact: true }).click()
    await page
      .locator('.ant-popconfirm')
      .getByRole('button', { name: 'Remove', exact: true })
      .click()
    await expect(
      page.getByText('User removed from group successfully'),
    ).toBeVisible({ timeout: 10000 })
    // The Member tag is gone; the inline Assign action is back.
    await expect(row.getByText('Member')).toHaveCount(0, { timeout: 10000 })
    await expect(row.getByRole('button', { name: 'Assign', exact: true })).toBeVisible()
  })
})
