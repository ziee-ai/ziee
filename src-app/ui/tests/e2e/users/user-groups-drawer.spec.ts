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
 * E2E — the UserGroupsDrawer in-list Assign / Remove controls
 * (`user/components/user/UserGroupsDrawer.tsx`). Drives both real endpoints
 * (POST /api/groups/assign, DELETE /api/groups/{user}/{group}/remove) through
 * the drawer UI and asserts the row's Member tag + Assign/Remove affordance
 * flip accordingly.
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

    await navigateToUserGroups(page, baseURL)
    await openCreateGroupDrawer(page)
    await createGroup(page, {
      name: groupName,
      description: 'user-groups-drawer e2e',
    })

    await navigateToUsers(page, baseURL)
    await openCreateUserDrawer(page)
    await createUser(page, {
      username,
      email: `${username}@example.com`,
      password: 'password123',
    })

    await openUserGroupsDrawer(page, username)
    const row = byTestId(page, `user-groups-drawer-row-${groupName}`)
    await expect(row).toBeVisible()

    // Not yet a member → shows Assign, no Member tag.
    await expect(
      row.locator('[data-testid^="user-groups-drawer-member-tag-"]'),
    ).toHaveCount(0)
    const assignBtn = row.locator(
      '[data-testid^="user-groups-drawer-assign-row-button-"]',
    )
    await expect(assignBtn).toBeVisible()

    // ---- ASSIGN: real POST /api/groups/assign ----
    const assignResp = page.waitForResponse(
      r =>
        /\/api\/groups\/assign$/.test(r.url()) &&
        r.request().method() === 'POST',
    )
    await assignBtn.click()
    expect((await assignResp).status()).toBeLessThan(400)

    // Row reflects membership: Member tag + Remove replace Assign.
    await expect(
      row.locator('[data-testid^="user-groups-drawer-member-tag-"]'),
    ).toBeVisible()
    const removeBtn = row.locator(
      '[data-testid^="user-groups-drawer-remove-button-"]',
    )
    await expect(removeBtn).toBeVisible()
    await expect(
      row.locator('[data-testid^="user-groups-drawer-assign-row-button-"]'),
    ).toHaveCount(0)

    // ---- REMOVE: real DELETE /api/groups/{user_id}/{group_id}/remove ----
    const removeResp = page.waitForResponse(
      r =>
        /\/api\/groups\/.+\/remove$/.test(r.url()) &&
        r.request().method() === 'DELETE',
    )
    await removeBtn.click()
    // Confirm the kit Confirm dialog (confirm button = `<testid>-confirm`).
    await page
      .locator(
        '[data-testid^="user-groups-drawer-remove-confirm-"][data-testid$="-confirm"]',
      )
      .click()
    expect((await removeResp).status()).toBeLessThan(400)

    // Row reverts to non-member: Member tag gone, Assign back.
    await expect(
      row.locator('[data-testid^="user-groups-drawer-member-tag-"]'),
    ).toHaveCount(0)
    await expect(
      row.locator('[data-testid^="user-groups-drawer-assign-row-button-"]'),
    ).toBeVisible()
  })
})
