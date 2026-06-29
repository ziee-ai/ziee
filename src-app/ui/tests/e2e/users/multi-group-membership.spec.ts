import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'
import { loginAsAdmin } from '../../common/auth-helpers'
import {
  navigateToUsers,
  navigateToUserGroups,
  openCreateUserDrawer,
  openCreateGroupDrawer,
  openUserGroupsDrawer,
} from './helpers/user-navigation'
import { createUser } from './helpers/user-actions'
import { createGroup, assignUserToGroupInDrawer } from './helpers/group-actions'
import { assertUserExists, assertGroupExists } from './helpers/user-assertions'

/**
 * E2E — a user assigned to 2+ groups. Verifies multi-group membership: both
 * groups show the Member tag in the user's Groups drawer after assigning the
 * user to two.
 */

test.describe('Users — multi-group membership', () => {
  test('a user assigned to two groups lists both in their Groups drawer', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)

    const ts = Date.now()
    const groupA = `MultiGrpA${ts}`
    const groupB = `MultiGrpB${ts}`
    const username = `multigrp${ts}`

    // Two groups.
    await navigateToUserGroups(page, baseURL)
    await openCreateGroupDrawer(page)
    await createGroup(page, { name: groupA, description: 'group A' })
    await assertGroupExists(page, groupA)
    await openCreateGroupDrawer(page)
    await createGroup(page, { name: groupB, description: 'group B' })
    await assertGroupExists(page, groupB)

    // One user.
    await navigateToUsers(page, baseURL)
    await openCreateUserDrawer(page)
    await createUser(page, {
      username,
      email: `${username}@example.com`,
      password: 'password123',
    })
    await assertUserExists(page, username)

    // Assign the user to BOTH groups via the per-row Assign controls.
    await openUserGroupsDrawer(page, username)
    await assignUserToGroupInDrawer(page, groupA)
    await assignUserToGroupInDrawer(page, groupB)

    // The user's Groups drawer now shows the Member tag on BOTH rows.
    await expect(
      byTestId(page, `user-groups-drawer-row-${groupA}`).locator(
        '[data-testid^="user-groups-drawer-member-tag-"]',
      ),
    ).toBeVisible({ timeout: 10000 })
    await expect(
      byTestId(page, `user-groups-drawer-row-${groupB}`).locator(
        '[data-testid^="user-groups-drawer-member-tag-"]',
      ),
    ).toBeVisible({ timeout: 10000 })
  })
})
