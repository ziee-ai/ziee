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

/**
 * E2E — the UserGroupsDrawer in-list Assign / Remove controls
 * (`user/components/user/UserGroupsDrawer.tsx`).
 *
 * Audit gap (r2-eaa3b4938e99): the sibling `05-assign-group-drawer.spec.ts`
 * drives the AssignGroupDrawer sub-drawer (reached via the "+" extra), but the
 * UserGroupsDrawer's OWN per-row "Assign" button (`handleAssignToGroup` →
 * `POST /api/groups/assign`) and "Remove" Popconfirm (`handleRemoveFromGroup`
 * → `DELETE /api/groups/{user_id}/{group_id}/remove`) had zero coverage. This
 * drives both real endpoints through the drawer UI and asserts the row's
 * Member tag + Assign/Remove affordance flip accordingly.
 */
test.describe('UserGroupsDrawer — in-list assign / remove', () => {
  test('assigns then removes a user via the per-row controls', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)

    const suffix = Date.now().toString(36)
    const groupName = `UGDrawer${suffix}`
    const username = `ugd_${suffix}`

    // Create the target group.
    await navigateToUserGroups(page, baseURL)
    await openCreateGroupDrawer(page)
    await createGroup(page, { name: groupName, description: 'user-groups-drawer e2e' })

    // Create the user.
    await navigateToUsers(page, baseURL)
    await openCreateUserDrawer(page)
    await createUser(page, {
      username,
      email: `${username}@example.com`,
      password: 'password123',
    })

    // Open the user's groups drawer (title "Groups for <username>").
    await openUserGroupsDrawer(page, username)
    const drawer = page.locator('.ant-drawer.ant-drawer-open', {
      hasText: new RegExp(`Groups for ${username}`),
    })
    await expect(drawer).toBeVisible()

    // The row for our group: not yet a member → shows "Assign", no Member tag.
    const row = drawer.locator('.ant-list-item', { hasText: groupName })
    await expect(row).toBeVisible()
    await expect(row.getByText('Member', { exact: true })).toHaveCount(0)
    const assignBtn = row.getByRole('button', { name: 'Assign', exact: true })
    await expect(assignBtn).toBeVisible()

    // ---- ASSIGN: real POST /api/groups/assign ----
    const assignResp = page.waitForResponse(
      r => /\/api\/groups\/assign$/.test(r.url()) && r.request().method() === 'POST',
    )
    await assignBtn.click()
    expect((await assignResp).status()).toBeLessThan(400)

    // Row reflects membership: Member tag + Remove control replace Assign.
    await expect(row.getByText('Member', { exact: true })).toBeVisible()
    const removeBtn = row.getByRole('button', { name: 'Remove', exact: true })
    await expect(removeBtn).toBeVisible()
    await expect(row.getByRole('button', { name: 'Assign', exact: true })).toHaveCount(0)

    // ---- REMOVE: real DELETE /api/groups/{user_id}/{group_id}/remove ----
    const removeResp = page.waitForResponse(
      r => /\/api\/groups\/.+\/remove$/.test(r.url()) && r.request().method() === 'DELETE',
    )
    await removeBtn.click()
    // Confirm the Popconfirm (its primary "Remove" button lives in the popover).
    const popover = page.locator('.ant-popover:visible').last()
    await expect(popover).toBeVisible()
    await popover.locator('.ant-btn-primary').click()
    expect((await removeResp).status()).toBeLessThan(400)

    // Row reverts to non-member: Member tag gone, Assign back.
    await expect(row.getByText('Member', { exact: true })).toHaveCount(0)
    await expect(
      row.getByRole('button', { name: 'Assign', exact: true }),
    ).toBeVisible()
  })
})
