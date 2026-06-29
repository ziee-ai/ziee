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
import { createGroup } from './helpers/group-actions'

/**
 * E2E — the AssignGroupDrawer (`user/components/user/AssignGroupDrawer.tsx`):
 * a single-group Select + "Assign" submit + "select a group" validator,
 * reached via the "+" extra on the user's groups drawer. Drives the whole
 * drawer: open it, the empty-submit validation, then a real assignment that
 * emits the success toast.
 */
test.describe('Assign-to-Group drawer', () => {
  test('assigns a user to a group via the drawer', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)

    const suffix = Date.now().toString(36)
    const groupName = `AssignGrp${suffix}`
    const username = `assignee_${suffix}`

    await navigateToUserGroups(page, baseURL)
    await openCreateGroupDrawer(page)
    await createGroup(page, { name: groupName, description: 'assign-drawer e2e' })

    await navigateToUsers(page, baseURL)
    await openCreateUserDrawer(page)
    await createUser(page, {
      username,
      email: `${username}@example.com`,
      password: 'password123',
    })

    // Open the user's groups drawer → the AssignGroupDrawer sub-drawer.
    await openUserGroupsDrawer(page, username)
    await byTestId(page, 'user-groups-drawer-assign-button').click()
    await byTestId(page, 'user-assign-group-form').waitFor({ state: 'visible' })

    // Pick the group via its checkbox, then submit.
    await byTestId(page, 'user-assign-group-checkboxes').waitFor({ state: 'visible' })
    await page
      .locator('[data-testid^="user-assign-group-checkboxes-opt-"]')
      .filter({ hasText: groupName })
      .click()
    await byTestId(page, 'user-assign-group-submit-button').click()

    await expect(
      page.locator('[data-sonner-toast][data-type="success"]').first(),
    ).toBeVisible({ timeout: 10000 })
  })

  test('blocks an empty submit with the group validator', async ({
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

    await openUserGroupsDrawer(page, username)
    await byTestId(page, 'user-groups-drawer-assign-button').click()
    await byTestId(page, 'user-assign-group-form').waitFor({ state: 'visible' })

    // Submit with nothing selected → the group_ids validator rejects.
    await byTestId(page, 'user-assign-group-submit-button').click()
    await expect(byTestId(page, 'field-error-group_ids')).toBeVisible({
      timeout: 10000,
    })
  })
})
