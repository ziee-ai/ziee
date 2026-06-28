import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import {
  navigateToUsers,
  navigateToUserGroups,
  openCreateUserDrawer,
  openCreateGroupDrawer,
  openUserGroupsDrawer,
} from './helpers/user-navigation'
import { createUser } from './helpers/user-actions'
import {
  createGroup,
  viewGroupMembers,
  removeUserFromGroup,
} from './helpers/group-actions'
import {
  assertUserExists,
  assertGroupExists,
  assertUserInGroup,
  assertUserNotInGroup,
} from './helpers/user-assertions'

/**
 * E2E — cross-entity workflow: create a user, create a group, assign the user
 * to the group, verify membership from BOTH the user-side groups drawer and the
 * group-side members drawer, then remove the membership.
 *
 * The existing 02-users specs cover user CRUD and group CRUD in isolation; this
 * stitches the full user↔group lifecycle together through the real UI.
 */

test.describe('Users ↔ Groups — end-to-end membership lifecycle', () => {
  test('create user + group, assign, verify both sides, then remove', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)

    const ts = Date.now()
    const groupName = `MembershipGroup${ts}`
    const username = `memberuser${ts}`

    // 1) Create the group.
    await navigateToUserGroups(page, baseURL)
    await openCreateGroupDrawer(page)
    await createGroup(page, { name: groupName, description: 'membership e2e' })
    await assertGroupExists(page, groupName)

    // 2) Create the user.
    await navigateToUsers(page, baseURL)
    await openCreateUserDrawer(page)
    await createUser(page, {
      username,
      email: `${username}@example.com`,
      password: 'password123',
    })
    await assertUserExists(page, username)

    // 3) Assign the user to the group via the user's Groups drawer.
    await openUserGroupsDrawer(page, username)
    await page.getByRole('button', { name: 'Assign group' }).click()
    const assignDrawer = page.locator(
      '.ant-drawer.ant-drawer-open:has-text("Assign to Group")',
    )
    await expect(assignDrawer).toBeVisible({ timeout: 10000 })
    await assignDrawer.getByRole('checkbox', { name: groupName }).check()
    await assignDrawer.getByRole('button', { name: 'Assign', exact: true }).click()
    await expect(page.locator('.ant-message-success').first()).toBeVisible({
      timeout: 10000,
    })

    // 4a) User-side: the group now appears in the user's groups drawer list.
    await expect(
      page
        .locator('.ant-drawer.ant-drawer-open')
        .locator('.ant-list-item', { hasText: groupName }),
    ).toBeVisible({ timeout: 10000 })

    // 4b) Group-side: the members drawer lists the user.
    await navigateToUserGroups(page, baseURL)
    await viewGroupMembers(page, groupName)
    await assertUserInGroup(page, username)

    // 5) Manage membership: remove the user from the group.
    await removeUserFromGroup(page, username)
    await assertUserNotInGroup(page, username)
  })
})
