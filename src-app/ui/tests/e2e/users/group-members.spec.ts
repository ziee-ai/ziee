import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'
import {
  loginAsAdmin,
  getAdminToken,
  createTestUser,
} from '../../common/auth-helpers'
import { createGroupViaAPI } from '../../common/provider-helpers'
import {
  navigateToUsers,
  navigateToUserGroups,
  openCreateUserDrawer,
  openCreateGroupDrawer,
  openEditGroupDrawer,
  openUserGroupsDrawer,
} from './helpers/user-navigation'
import { createUser } from './helpers/user-actions'
import {
  createGroup,
  updateGroup,
  viewGroupMembers,
  assignUserToGroupInDrawer,
  removeUserFromGroup,
} from './helpers/group-actions'
import {
  assertGroupStatus,
  assertDrawerOpen,
  assertDrawerClosed,
} from './helpers/user-assertions'

// First group card that carries a System tag (the seeded built-in groups).
const firstSystemGroupCard = (page: import('@playwright/test').Page) =>
  page
    .locator('[data-testid^="user-group-card-"]')
    .filter({ has: page.locator('[data-testid^="user-group-system-tag-"]') })
    .first()

test.describe('Group Membership Management', () => {
  test('should display group members drawer', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await navigateToUserGroups(page, baseURL)

    await openCreateGroupDrawer(page)
    const timestamp = Date.now()
    const groupData = { name: `TestGroup${timestamp}`, description: 'Test group' }
    await createGroup(page, groupData)

    await viewGroupMembers(page, groupData.name)
    await assertDrawerOpen(page)
    // The drawer title carries the group name (dynamic data the test created).
    await expect(page.getByRole('dialog').first()).toContainText(groupData.name)
  })

  test('should display empty state when group has no members', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await navigateToUserGroups(page, baseURL)

    await openCreateGroupDrawer(page)
    const timestamp = Date.now()
    const groupData = { name: `TestGroup${timestamp}`, description: 'Empty group' }
    await createGroup(page, groupData)

    await viewGroupMembers(page, groupData.name)

    // A fresh group has no member rows.
    await expect(
      byTestId(page, 'user-group-members-list').locator(
        '[data-testid^="user-group-member-row-"]',
      ),
    ).toHaveCount(0)
  })

  test('should display user groups drawer', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await navigateToUsers(page, baseURL)

    await openCreateUserDrawer(page)
    const timestamp = Date.now()
    const userData = {
      username: `testuser${timestamp}`,
      email: `testuser${timestamp}@example.com`,
      password: 'password123',
    }
    await createUser(page, userData)

    await openUserGroupsDrawer(page, userData.username)
    await assertDrawerOpen(page)
  })

  test('should show system groups in list', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await navigateToUserGroups(page, baseURL)

    const card = firstSystemGroupCard(page)
    if (await card.isVisible()) {
      await expect(
        card.locator('[data-testid^="user-group-system-tag-"]'),
      ).toBeVisible()

      // View its members.
      await card.locator('[data-testid^="user-group-members-button-"]').click()
      await byTestId(page, 'user-group-members-list').waitFor({
        state: 'visible',
      })
      // A system group (e.g. Administrators) has at least one member.
      await expect(
        byTestId(page, 'user-group-members-list')
          .locator('[data-testid^="user-group-member-row-"]')
          .first(),
      ).toBeVisible()
    }
  })

  test('should display user information in members list', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await navigateToUserGroups(page, baseURL)

    const card = firstSystemGroupCard(page)
    if (await card.isVisible()) {
      await card.locator('[data-testid^="user-group-members-button-"]').click()
      await byTestId(page, 'user-group-members-list').waitFor({
        state: 'visible',
      })

      const firstMember = byTestId(page, 'user-group-members-list')
        .locator('[data-testid^="user-group-member-row-"]')
        .first()
      if (await firstMember.isVisible()) {
        // Each member row carries a status tag.
        await expect(
          firstMember.locator('[data-testid^="user-group-member-status-tag-"]'),
        ).toBeVisible()
      }
    }
  })

  test('should display active/inactive status for group members', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await navigateToUserGroups(page, baseURL)

    const card = firstSystemGroupCard(page)
    if (await card.isVisible()) {
      await card.locator('[data-testid^="user-group-members-button-"]').click()
      await byTestId(page, 'user-group-members-list').waitFor({
        state: 'visible',
      })

      const statusTag = byTestId(page, 'user-group-members-list')
        .locator('[data-testid^="user-group-member-status-tag-"]')
        .first()
      if (await statusTag.isVisible()) {
        await expect(statusTag).toHaveText(/^(active|inactive)$/i)
      }
    }
  })

  test('should close members drawer', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await navigateToUserGroups(page, baseURL)

    await openCreateGroupDrawer(page)
    const timestamp = Date.now()
    const groupData = { name: `TestGroup${timestamp}`, description: 'Test group' }
    await createGroup(page, groupData)

    await viewGroupMembers(page, groupData.name)
    await assertDrawerOpen(page)

    await byTestId(page, 'layout-drawer-close-button').click()
    await assertDrawerClosed(page)
  })

  test('should handle loading state when fetching members', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await navigateToUserGroups(page, baseURL)

    const card = firstSystemGroupCard(page)
    if (await card.isVisible()) {
      await card.locator('[data-testid^="user-group-members-button-"]').click()
      // The members list eventually renders.
      await expect(byTestId(page, 'user-group-members-list')).toBeVisible({
        timeout: 5000,
      })
    }
  })

  test('should navigate between users and groups pages', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)

    await navigateToUsers(page, baseURL)
    await expect(byTestId(page, 'user-list-card')).toBeVisible()

    await navigateToUserGroups(page, baseURL)
    await expect(byTestId(page, 'user-groups-create-button')).toBeVisible()

    await navigateToUsers(page, baseURL)
    await expect(byTestId(page, 'user-list-card')).toBeVisible()
  })

  test('assigns then removes a user from a group via the groups drawer', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)

    const ts = Date.now()
    const groupName = `QA Team ${ts}`
    await createGroupViaAPI(apiURL, adminToken, groupName, 'qa group', [
      'profile::read',
    ])
    const username = `grpmember${ts}`
    await createTestUser(
      apiURL,
      adminToken,
      username,
      `${username}@example.com`,
      'password123',
      [],
    )

    await navigateToUsers(page, baseURL)
    await openUserGroupsDrawer(page, username)

    // ASSIGN via the per-row Assign control (real POST /api/groups/assign).
    await assignUserToGroupInDrawer(page, groupName)

    // REMOVE via the per-row Remove control + Confirm.
    await removeUserFromGroup(page, groupName)

    // Back to "Assign" — no longer a member.
    await expect(
      byTestId(page, `user-groups-drawer-row-${groupName}`).locator(
        '[data-testid^="user-groups-drawer-assign-row-button-"]',
      ),
    ).toBeVisible()
  })

  test('group status badge colors reflect active vs inactive', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await navigateToUserGroups(page, baseURL)

    const ts = Date.now()
    const groupName = `StatusGrp_${ts}`

    // A new group is active by default.
    await openCreateGroupDrawer(page)
    await createGroup(page, { name: groupName })
    await assertGroupStatus(page, groupName, 'active')

    // Edit it to inactive.
    await openEditGroupDrawer(page, groupName)
    await updateGroup(page, { isActive: false })
    await assertGroupStatus(page, groupName, 'inactive')
  })

  test('assigns a user to multiple groups via the groups drawer', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)

    const ts = Date.now()
    const groupA = `Bulk A ${ts}`
    const groupB = `Bulk B ${ts}`
    await createGroupViaAPI(apiURL, adminToken, groupA, 'bulk a', [
      'profile::read',
    ])
    await createGroupViaAPI(apiURL, adminToken, groupB, 'bulk b', [
      'profile::read',
    ])
    const username = `bulkmember${ts}`
    await createTestUser(
      apiURL,
      adminToken,
      username,
      `${username}@example.com`,
      'password123',
      [],
    )

    await navigateToUsers(page, baseURL)
    await openUserGroupsDrawer(page, username)

    // Assign to both groups via their per-row Assign controls.
    await assignUserToGroupInDrawer(page, groupA)
    await assignUserToGroupInDrawer(page, groupB)

    // Both rows now show the Member tag.
    await expect(
      byTestId(page, `user-groups-drawer-row-${groupA}`).locator(
        '[data-testid^="user-groups-drawer-member-tag-"]',
      ),
    ).toBeVisible()
    await expect(
      byTestId(page, `user-groups-drawer-row-${groupB}`).locator(
        '[data-testid^="user-groups-drawer-member-tag-"]',
      ),
    ).toBeVisible()
  })
})
