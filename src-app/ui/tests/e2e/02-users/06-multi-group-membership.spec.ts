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
import { createGroup } from './helpers/group-actions'
import { assertUserExists, assertGroupExists } from './helpers/user-assertions'

/**
 * E2E — a user assigned to 2+ groups. The existing membership specs only ever
 * put a user in a single group; this verifies multi-group membership: both
 * groups are shown in the user's Groups drawer after assigning the user to two.
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

    // Assign the user to BOTH groups via the AssignGroupDrawer (multi-check).
    await openUserGroupsDrawer(page, username)
    await page.getByRole('button', { name: 'Assign group' }).click()
    const assignDrawer = page.locator(
      '.ant-drawer.ant-drawer-open:has-text("Assign to Group")',
    )
    await expect(assignDrawer).toBeVisible({ timeout: 10000 })
    await assignDrawer.getByRole('checkbox', { name: groupA }).check()
    await assignDrawer.getByRole('checkbox', { name: groupB }).check()
    await assignDrawer.getByRole('button', { name: 'Assign', exact: true }).click()
    await expect(page.locator('.ant-message-success').first()).toBeVisible({
      timeout: 10000,
    })

    // The user's Groups drawer now lists BOTH groups.
    const drawer = page.locator('.ant-drawer.ant-drawer-open')
    await expect(
      drawer.locator('.ant-list-item', { hasText: groupA }),
    ).toBeVisible({ timeout: 10000 })
    await expect(
      drawer.locator('.ant-list-item', { hasText: groupB }),
    ).toBeVisible({ timeout: 10000 })
  })
})
