import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import {
  navigateToUsers,
  navigateToUserGroups,
  openCreateUserDrawer,
  openCreateGroupDrawer,
  openGroupMembersDrawer as _openGroupMembersDrawer,
  openUserGroupsDrawer,
} from './helpers/user-navigation'
import { createUser, assignUserToGroups } from './helpers/user-actions'
import { createGroup, viewGroupMembers } from './helpers/group-actions'
import {
  assertUserExists as _assertUserExists,
  assertGroupExists as _assertGroupExists,
  assertUserInGroup as _assertUserInGroup,
  assertUserNotInGroup as _assertUserNotInGroup,
  assertDrawerOpen,
} from './helpers/user-assertions'

test.describe('Group Membership Management', () => {
  test('should display group members drawer', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)

    // Navigate to groups page
    await navigateToUserGroups(page, baseURL)

    // Create a test group
    await openCreateGroupDrawer(page)
    const timestamp = Date.now()
    const groupData = {
      name: `TestGroup${timestamp}`,
      description: 'Test group',
    }
    await createGroup(page, groupData)

    // Open members drawer
    await viewGroupMembers(page, groupData.name)

    // Verify drawer opened
    await assertDrawerOpen(page, /members of/i)

    // Verify group name is in drawer title
    const drawer = page.locator('.ant-drawer.ant-drawer-open')
    await expect(drawer.locator('.ant-drawer-title')).toContainText(
      groupData.name
    )
  })

  test('should display empty state when group has no members', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)

    // Navigate to groups page
    await navigateToUserGroups(page, baseURL)

    // Create a test group
    await openCreateGroupDrawer(page)
    const timestamp = Date.now()
    const groupData = {
      name: `TestGroup${timestamp}`,
      description: 'Empty group',
    }
    await createGroup(page, groupData)

    // Open members drawer
    await viewGroupMembers(page, groupData.name)

    // Verify empty state or no items message
    const drawer = page.locator('.ant-drawer.ant-drawer-open')
    const membersList = drawer.locator('.ant-list')

    // Either no items or empty state should be shown
    const isEmpty =
      (await membersList.locator('.ant-list-item').count()) === 0 ||
      (await drawer.locator('.ant-empty').isVisible())

    expect(isEmpty).toBe(true)
  })

  test('should display user groups drawer', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)

    // Navigate to users page
    await navigateToUsers(page, baseURL)

    // Create a test user
    await openCreateUserDrawer(page)
    const timestamp = Date.now()
    const userData = {
      username: `testuser${timestamp}`,
      email: `testuser${timestamp}@example.com`,
      password: 'password123',
    }
    await createUser(page, userData)

    // Open user groups drawer
    await openUserGroupsDrawer(page, userData.username)

    // Verify drawer opened
    const drawer = page.locator('.ant-drawer.ant-drawer-open')
    await expect(drawer).toBeVisible()
  })

  test('should show system groups in list', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)

    // Navigate to groups page
    await navigateToUserGroups(page, baseURL)

    // Check if admin group exists (it's typically a system group)
    const adminGroup = page.locator('.ant-card', { hasText: /admin/i }).first()
    const adminExists = await adminGroup.isVisible()

    if (adminExists) {
      // Verify it has system tag
      const systemTag = adminGroup.locator('.ant-tag', { hasText: /system/i }).first()
      await expect(systemTag).toBeVisible()

      // View members
      await viewGroupMembers(page, 'admin')

      // Verify members drawer opens
      await assertDrawerOpen(page, /members of/i)

      // Admin group should have at least one member
      const drawer = page.locator('.ant-drawer.ant-drawer-open')
      const membersList = drawer.locator('.ant-list-item')
      await expect(membersList.first()).toBeVisible()
    }
  })

  test('should display user information in members list', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)

    // Navigate to groups page
    await navigateToUserGroups(page, baseURL)

    // Find admin group (it typically has members)
    const adminGroup = page.locator('.ant-card', { hasText: /admin/i }).first()
    const adminExists = await adminGroup.isVisible()

    if (adminExists) {
      // View admin group members
      await viewGroupMembers(page, 'admin')

      // Verify drawer shows member information
      const drawer = page.locator('.ant-drawer.ant-drawer-open')
      const firstMember = drawer.locator('.ant-list-item').first()

      if (await firstMember.isVisible()) {
        // Check for username/title
        await expect(firstMember.locator('.ant-list-item-meta-title')).toBeVisible()

        // Check for email in description
        await expect(firstMember.locator('.ant-list-item-meta-description')).toBeVisible()

        // Check for status tag
        const statusTag = firstMember.locator('.ant-tag')
        await expect(statusTag).toBeVisible()
      }
    }
  })

  test('should display active/inactive status for group members', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)

    // Navigate to groups page
    await navigateToUserGroups(page, baseURL)

    // Find admin group
    const adminGroup = page.locator('.ant-card', { hasText: /admin/i }).first()
    const adminExists = await adminGroup.isVisible()

    if (adminExists) {
      await viewGroupMembers(page, 'admin')

      const drawer = page.locator('.ant-drawer.ant-drawer-open')
      const firstMember = drawer.locator('.ant-list-item').first()

      if (await firstMember.isVisible()) {
        // Check for status tag (green for active, red for inactive)
        const statusTag = firstMember.locator('.ant-tag')
        await expect(statusTag).toBeVisible()

        const statusText = await statusTag.textContent()
        expect(
          statusText?.toLowerCase() === 'active' ||
            statusText?.toLowerCase() === 'inactive'
        ).toBe(true)
      }
    }
  })

  test('should close members drawer', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)

    // Navigate to groups page
    await navigateToUserGroups(page, baseURL)

    // Create a test group
    await openCreateGroupDrawer(page)
    const timestamp = Date.now()
    const groupData = {
      name: `TestGroup${timestamp}`,
      description: 'Test group',
    }
    await createGroup(page, groupData)

    // Open members drawer
    await viewGroupMembers(page, groupData.name)
    await assertDrawerOpen(page, /members of/i)

    // Close drawer — the custom Drawer wrapper renders an aria-labelled
    // button in the title slot instead of the default .ant-drawer-close.
    const closeButton = page
      .locator('.ant-drawer.ant-drawer-open')
      .getByRole('button', { name: 'Close drawer' })
    await closeButton.click()

    // Verify drawer closed
    await page.waitForTimeout(300)
    const drawer = page.locator('.ant-drawer.ant-drawer-open')
    await expect(drawer).not.toBeVisible()
  })

  test('should handle loading state when fetching members', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)

    // Navigate to groups page
    await navigateToUserGroups(page, baseURL)

    // Find admin group
    const adminGroup = page.locator('.ant-card', { hasText: /admin/i }).first()
    const adminExists = await adminGroup.isVisible()

    if (adminExists) {
      // Click members button (admin card contains a nested card so
      // members buttons can appear at multiple depths — take first).
      const membersButton = adminGroup.getByRole('button', {
        name: /members/i,
      }).first()
      await membersButton.click()

      // Wait for drawer to appear
      const drawer = page.locator('.ant-drawer.ant-drawer-open')
      await drawer.waitFor({ state: 'visible' })

      // Loading spinner should appear briefly (or list loads quickly)
      const spinner = drawer.locator('.ant-spin')
      // Spinner might not be visible if data loads too quickly, that's okay

      // Eventually, either spinner disappears or list appears. The drawer
      // can hold more than one `.ant-list` / `.ant-spin` (members list plus
      // an add-members list, antd v6's Spin wrapper), so assert that at
      // least one is visible rather than tripping strict mode.
      await expect(
        drawer.locator('.ant-list').or(spinner).first()
      ).toBeVisible({ timeout: 5000 })
    }
  })

  test('should navigate between users and groups pages', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)

    // Start on users page — page heading is level 4, not h1.
    await navigateToUsers(page, baseURL)
    await expect(
      page.getByRole('heading', { name: /^users$/i, level: 4 })
    ).toBeVisible()

    // Navigate to groups page
    await navigateToUserGroups(page, baseURL)
    await expect(
      page.getByRole('heading', { name: /user groups/i, level: 4 })
    ).toBeVisible()

    // Navigate back to users page
    await navigateToUsers(page, baseURL)
    await expect(
      page.getByRole('heading', { name: /^users$/i, level: 4 })
    ).toBeVisible()
  })

  // audit id 658cfc658b378128 — the AssignGroupDrawer (Assign-to-Group flow)
  // had zero E2E coverage. Create a group + user, open the user's groups drawer,
  // open the Assign drawer, check the group, submit, and verify membership.
  test('assigns a user to a group via the AssignGroupDrawer', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const ts = Date.now()
    const groupName = `AssignGrp${ts}`
    const username = `assignuser${ts}`

    // Create the target group.
    await navigateToUserGroups(page, baseURL)
    await openCreateGroupDrawer(page)
    await createGroup(page, { name: groupName, description: 'assign target' })

    // Create the user.
    await navigateToUsers(page, baseURL)
    await openCreateUserDrawer(page)
    await createUser(page, {
      username,
      email: `${username}@example.com`,
      password: 'password123',
    })

    // Drive the AssignGroupDrawer (open user groups → Assign → check group →
    // submit → success toast). The helper asserts the success message.
    await assignUserToGroups(page, username, [groupName])

    // Verify the membership took: the user shows in the group's members list.
    await navigateToUserGroups(page, baseURL)
    await viewGroupMembers(page, groupName)
    await _assertUserInGroup(page, username)
  })
})
