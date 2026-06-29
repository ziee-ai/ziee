import { test } from '../../fixtures/test-context'
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
  assignUserToGroupInDrawer,
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

    // 3) Assign the user to the group via the per-row control in the groups
    //    drawer.
    await openUserGroupsDrawer(page, username)
    await assignUserToGroupInDrawer(page, groupName)

    // 4) Group-side: the members drawer lists the user.
    await navigateToUserGroups(page, baseURL)
    await viewGroupMembers(page, groupName)
    await assertUserInGroup(page, username)

    // 5) Remove membership via the user's groups drawer + verify gone.
    await navigateToUsers(page, baseURL)
    await openUserGroupsDrawer(page, username)
    await removeUserFromGroup(page, groupName)

    await navigateToUserGroups(page, baseURL)
    await viewGroupMembers(page, groupName)
    await assertUserNotInGroup(page, username)
  })
})
