import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import {
  navigateToUsers,
  navigateToUserGroups,
  openCreateUserDrawer,
  openCreateGroupDrawer,
  openUserGroupsDrawer,
} from './helpers/user-navigation'
import { createUser, assignUserToGroups } from './helpers/user-actions'
import { createGroup } from './helpers/group-actions'

/**
 * E2E — the AssignGroupDrawer (`user/components/user/AssignGroupDrawer.tsx`).
 *
 * Audit gap: the component (checkbox group of all user-groups + "Assign"
 * submit + "select at least one group" validator) had ZERO E2E coverage — a
 * helper (`assignUserToGroups`) existed but no spec invoked it. This spec
 * drives the whole drawer: open from a user's groups drawer, the empty-submit
 * validation, then a real assignment that emits the success message.
 */
test.describe('Assign-to-Group drawer', () => {
  test('assigns a user to a group via the drawer', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)

    const suffix = Date.now().toString(36)
    const groupName = `AssignGrp${suffix}`
    const username = `assignee_${suffix}`

    // Create the target group.
    await navigateToUserGroups(page, baseURL)
    await openCreateGroupDrawer(page)
    await createGroup(page, { name: groupName, description: 'assign-drawer e2e' })

    // Create the user to assign.
    await navigateToUsers(page, baseURL)
    await openCreateUserDrawer(page)
    await createUser(page, {
      username,
      email: `${username}@example.com`,
      password: 'password123',
    })

    // Run the assignment through the drawer; helper asserts the success toast.
    await assignUserToGroups(page, username, [groupName])
  })

  test('blocks an empty submit with the "select at least one group" validator', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)

    const suffix = Date.now().toString(36)
    const username = `assignee2_${suffix}`

    await navigateToUsers(page, baseURL)
    await openCreateUserDrawer(page)
    await createUser(page, {
      username,
      email: `${username}@example.com`,
      password: 'password123',
    })

    // Open the user's groups drawer, then the Assign-to-Group sub-drawer.
    await openUserGroupsDrawer(page, username)
    const groupsDrawer = page.locator('.ant-drawer.ant-drawer-open')
    await groupsDrawer.getByRole('button', { name: /assign.*group/i }).click()

    // Submit with nothing selected → the Form.Item validator rejects.
    const assignDrawer = page.locator('.ant-drawer.ant-drawer-open').last()
    await assignDrawer
      .locator('.ant-btn-primary[type="submit"]')
      .click()
    await expect(
      page.getByText('Please select at least one group'),
    ).toBeVisible({ timeout: 10000 })
  })
})
