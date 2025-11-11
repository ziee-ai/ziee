import { test, expect } from '../../fixtures/test-context'
import { assertNoAccessibilityViolations } from '../../utils/accessibility'
import { loginAsAdmin } from '../../common/auth-helpers'
import {
  goToUserGroupsPage,
  createUserGroup,
  deleteUserGroup,
  openGroupAssignmentDrawerFromServer,
  toggleGroupInDrawer,
  saveGroupAssignment,
  cancelGroupAssignment,
  assignGroupToServer,
  removeGroupFromServer,
  assertGroupInServerCard,
  assertGroupNotInServerCard,
  assertServerCardShowsCount,
} from './helpers/group-server-helpers'
import { createSystemServer, deleteSystemServer } from './helpers/server-helpers'
import { goToMcpAdminPage, clickServerCard } from './helpers/navigation-helpers'

test.describe('User Group Assignment in MCP Servers', () => {
  test('should pass accessibility checks on server detail page with card', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    const groupName = `test-group-a11y-${Date.now()}`
    const serverName = `test-server-a11y-${Date.now()}`
    const serverDisplayName = `Test Server A11y ${Date.now()}`

    await loginAsAdmin(page, baseURL)

    // Setup: Create group and server
    await createUserGroup(page, baseURL, groupName, 'Accessibility test group')
    await createSystemServer(page, baseURL, serverName, serverDisplayName, 'Accessibility test server')

    // Navigate to server detail page
    await goToMcpAdminPage(page, baseURL)
    await clickServerCard(page, serverDisplayName, true)

    // Check accessibility
    await assertNoAccessibilityViolations(page)

    // Cleanup
    await goToMcpAdminPage(page, baseURL)
    await deleteSystemServer(page, serverDisplayName)
    await goToUserGroupsPage(page, baseURL)
    await deleteUserGroup(page, groupName)
  })

  test('should display User Groups card in server detail page', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    const serverName = `test-server-card-${Date.now()}`
    const serverDisplayName = `Test Server Card ${Date.now()}`

    await loginAsAdmin(page, baseURL)
    await createSystemServer(page, baseURL, serverName, serverDisplayName, 'Card display test')

    // Navigate to admin page (cards are inline, not on separate detail page)
    await goToMcpAdminPage(page, baseURL)
    await clickServerCard(page, serverDisplayName, true)

    // Find the container using filter() to avoid strict mode violations
    const containers = page.locator('div.flex.flex-col.gap-3')
    const container = containers.filter({
      has: page.locator(`.ant-card:has-text("${serverDisplayName}")`)
    }).last()
    await container.scrollIntoViewIfNeeded()
    await page.waitForTimeout(500)

    const card = container.locator('.ant-card:has(.ant-card-head-title:has-text("User Groups"))')
    await expect(card).toBeVisible({ timeout: 15000 })

    // Verify edit button exists
    const editButton = card.locator('button[aria-label="Manage user groups"]')
    await expect(editButton).toBeVisible()

    // Cleanup
    await goToMcpAdminPage(page, baseURL)
    await deleteSystemServer(page, serverDisplayName)
  })

  test('should show empty state when no groups assigned', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    const serverName = `test-server-empty-${Date.now()}`
    const serverDisplayName = `Test Server Empty ${Date.now()}`

    await loginAsAdmin(page, baseURL)
    await createSystemServer(page, baseURL, serverName, serverDisplayName, 'Empty state test')

    // Navigate to server detail page
    await goToMcpAdminPage(page, baseURL)
    await clickServerCard(page, serverDisplayName, true)

    // Verify empty state
    await assertServerCardShowsCount(page, 0, serverDisplayName)

    // Cleanup
    await goToMcpAdminPage(page, baseURL)
    await deleteSystemServer(page, serverDisplayName)
  })

  test('should open group assignment drawer from server card', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    const groupName = `test-group-drawer-${Date.now()}`
    const serverName = `test-server-drawer-${Date.now()}`
    const serverDisplayName = `Test Server Drawer ${Date.now()}`

    await loginAsAdmin(page, baseURL)

    // Setup
    await createUserGroup(page, baseURL, groupName, 'Drawer test group')
    await createSystemServer(page, baseURL, serverName, serverDisplayName, 'Drawer test server')

    // Navigate to admin page
    await goToMcpAdminPage(page, baseURL)
    await clickServerCard(page, serverDisplayName, true)

    // Open drawer (pass serverDisplayName to find correct card)
    await openGroupAssignmentDrawerFromServer(page, serverDisplayName)

    // Verify drawer is open with correct title
    await expect(
      page.locator(`.ant-drawer-title:has-text("Assign User Groups - ${serverDisplayName}")`)
    ).toBeVisible()

    // Verify group appears in the drawer
    await expect(page.locator(`.ant-drawer:visible:has-text("${groupName}")`)).toBeVisible()

    // Verify switch exists
    const groupCard = page.locator(
      `.ant-drawer:visible .ant-card:has(strong:has-text("${groupName}"))`
    )
    const switchElement = groupCard.locator('.ant-switch')
    await expect(switchElement).toBeVisible()

    // Close drawer
    await cancelGroupAssignment(page)

    // Cleanup
    await goToMcpAdminPage(page, baseURL)
    await deleteSystemServer(page, serverDisplayName)
    await goToUserGroupsPage(page, baseURL)
    await deleteUserGroup(page, groupName)
  })

  test('should assign group to server', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    const groupName = `test-group-assign-${Date.now()}`
    const serverName = `test-server-assign-${Date.now()}`
    const serverDisplayName = `Test Server Assign ${Date.now()}`

    await loginAsAdmin(page, baseURL)

    // Setup
    await createUserGroup(page, baseURL, groupName, 'Assignment test group')
    await createSystemServer(page, baseURL, serverName, serverDisplayName, 'Assignment test server')

    // Navigate to server detail page
    await goToMcpAdminPage(page, baseURL)
    await clickServerCard(page, serverDisplayName, true)

    // Get server ID
    const url = page.url()
    const serverId = url.split('/').pop()

    // Assign group
    await assignGroupToServer(page, serverId!, groupName, serverDisplayName)

    // Verify group appears in card
    await assertGroupInServerCard(page, groupName, serverDisplayName)

    // Cleanup
    await goToMcpAdminPage(page, baseURL)
    await deleteSystemServer(page, serverDisplayName)
    await goToUserGroupsPage(page, baseURL)
    await deleteUserGroup(page, groupName)
  })

  test('should remove group from server', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    const groupName = `test-group-remove-${Date.now()}`
    const serverName = `test-server-remove-${Date.now()}`
    const serverDisplayName = `Test Server Remove ${Date.now()}`

    await loginAsAdmin(page, baseURL)

    // Setup
    await createUserGroup(page, baseURL, groupName, 'Removal test group')
    await createSystemServer(page, baseURL, serverName, serverDisplayName, 'Removal test server')

    // Navigate to server detail page
    await goToMcpAdminPage(page, baseURL)
    await clickServerCard(page, serverDisplayName, true)

    // Get server ID
    const url = page.url()
    const serverId = url.split('/').pop()

    // Assign then remove
    await assignGroupToServer(page, serverId!, groupName, serverDisplayName)
    await removeGroupFromServer(page, serverId!, groupName, serverDisplayName)

    // Verify group is gone from card
    await assertGroupNotInServerCard(page, groupName, serverDisplayName)

    // Cleanup
    await goToMcpAdminPage(page, baseURL)
    await deleteSystemServer(page, serverDisplayName)
    await goToUserGroupsPage(page, baseURL)
    await deleteUserGroup(page, groupName)
  })

  test('should assign multiple groups to server', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    const group1 = `test-group-1-${Date.now()}`
    const group2 = `test-group-2-${Date.now()}`
    const serverName = `test-server-multi-${Date.now()}`
    const serverDisplayName = `Test Server Multi ${Date.now()}`

    await loginAsAdmin(page, baseURL)

    // Setup
    await createUserGroup(page, baseURL, group1, 'Group 1')
    await createUserGroup(page, baseURL, group2, 'Group 2')
    await createSystemServer(page, baseURL, serverName, serverDisplayName, 'Multiple groups test')

    // Navigate to server detail page
    await goToMcpAdminPage(page, baseURL)
    await clickServerCard(page, serverDisplayName, true)

    // Assign both groups at once
    await openGroupAssignmentDrawerFromServer(page, serverDisplayName)
    await toggleGroupInDrawer(page, group1, true)
    await toggleGroupInDrawer(page, group2, true)
    await saveGroupAssignment(page)

    // Verify both appear in card
    await assertGroupInServerCard(page, group1, serverDisplayName)
    await assertGroupInServerCard(page, group2, serverDisplayName)
    await assertServerCardShowsCount(page, 2, serverDisplayName)

    // Cleanup
    await goToMcpAdminPage(page, baseURL)
    await deleteSystemServer(page, serverDisplayName)
    await goToUserGroupsPage(page, baseURL)
    await deleteUserGroup(page, group1)
    await deleteUserGroup(page, group2)
  })

  test('should show default groups with tag', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    const serverName = `test-server-default-${Date.now()}`
    const serverDisplayName = `Test Server Default ${Date.now()}`

    await loginAsAdmin(page, baseURL)

    // Setup
    await createSystemServer(page, baseURL, serverName, serverDisplayName, 'Default groups test')

    // Navigate to server detail page
    await goToMcpAdminPage(page, baseURL)
    await clickServerCard(page, serverDisplayName, true)

    // Open drawer
    await openGroupAssignmentDrawerFromServer(page, serverDisplayName)

    // Look for "All Users" (which is a default group)
    const allUsersCard = page.locator(
      `.ant-drawer:visible .ant-card:has(strong:has-text("All Users"))`
    )

    // If All Users exists, verify it has Default tag
    const allUsersCount = await allUsersCard.count()
    if (allUsersCount > 0) {
      await expect(allUsersCard.locator('.ant-tag:has-text("Default")')).toBeVisible()
    }

    // Close drawer
    await cancelGroupAssignment(page)

    // Cleanup
    await goToMcpAdminPage(page, baseURL)
    await deleteSystemServer(page, serverDisplayName)
  })

  test('should update card count when groups are added/removed', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    const group1 = `test-group-count-1-${Date.now()}`
    const group2 = `test-group-count-2-${Date.now()}`
    const serverName = `test-server-count-${Date.now()}`
    const serverDisplayName = `Test Server Count ${Date.now()}`

    await loginAsAdmin(page, baseURL)

    // Setup
    await createUserGroup(page, baseURL, group1, 'Group 1')
    await createUserGroup(page, baseURL, group2, 'Group 2')
    await createSystemServer(page, baseURL, serverName, serverDisplayName, 'Count update test')

    // Navigate to server detail page
    await goToMcpAdminPage(page, baseURL)
    await clickServerCard(page, serverDisplayName, true)

    // Get server ID
    const url = page.url()
    const serverId = url.split('/').pop()

    // Start with 0
    await assertServerCardShowsCount(page, 0, serverDisplayName)

    // Add one group -> count = 1
    await assignGroupToServer(page, serverId!, group1, serverDisplayName)
    await assertServerCardShowsCount(page, 1, serverDisplayName)

    // Add another group -> count = 2
    await assignGroupToServer(page, serverId!, group2, serverDisplayName)
    await assertServerCardShowsCount(page, 2, serverDisplayName)

    // Remove one group -> count = 1
    await removeGroupFromServer(page, serverId!, group1, serverDisplayName)
    await assertServerCardShowsCount(page, 1, serverDisplayName)

    // Remove last group -> count = 0
    await removeGroupFromServer(page, serverId!, group2, serverDisplayName)
    await assertServerCardShowsCount(page, 0, serverDisplayName)

    // Cleanup
    await goToMcpAdminPage(page, baseURL)
    await deleteSystemServer(page, serverDisplayName)
    await goToUserGroupsPage(page, baseURL)
    await deleteUserGroup(page, group1)
    await deleteUserGroup(page, group2)
  })

  test('should cancel assignment without saving changes', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    const groupName = `test-group-cancel-${Date.now()}`
    const serverName = `test-server-cancel-${Date.now()}`
    const serverDisplayName = `Test Server Cancel ${Date.now()}`

    await loginAsAdmin(page, baseURL)

    // Setup
    await createUserGroup(page, baseURL, groupName, 'Cancel test group')
    await createSystemServer(page, baseURL, serverName, serverDisplayName, 'Cancel test server')

    // Navigate to server detail page
    await goToMcpAdminPage(page, baseURL)
    await clickServerCard(page, serverDisplayName, true)

    // Open drawer and toggle group but cancel
    await openGroupAssignmentDrawerFromServer(page, serverDisplayName)
    await toggleGroupInDrawer(page, groupName, true)
    await cancelGroupAssignment(page)

    // Verify group was NOT assigned
    await assertGroupNotInServerCard(page, groupName, serverDisplayName)

    // Cleanup
    await goToMcpAdminPage(page, baseURL)
    await deleteSystemServer(page, serverDisplayName)
    await goToUserGroupsPage(page, baseURL)
    await deleteUserGroup(page, groupName)
  })

  test('should toggle group by clicking switch', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    const groupName = `test-group-click-${Date.now()}`
    const serverName = `test-server-click-${Date.now()}`
    const serverDisplayName = `Test Server Click ${Date.now()}`

    await loginAsAdmin(page, baseURL)

    // Setup
    await createUserGroup(page, baseURL, groupName, 'Click test group')
    await createSystemServer(page, baseURL, serverName, serverDisplayName, 'Click test server')

    // Navigate to server detail page
    await goToMcpAdminPage(page, baseURL)
    await clickServerCard(page, serverDisplayName, true)

    // Open drawer
    await openGroupAssignmentDrawerFromServer(page, serverDisplayName)

    // Get the group card and switch
    const groupCard = page.locator(
      `.ant-drawer:visible .ant-card:has(strong:has-text("${groupName}"))`
    )
    const switchElement = groupCard.locator('.ant-switch')

    // Verify initially unchecked
    await expect(switchElement).toHaveAttribute('aria-checked', 'false')

    // Click the switch to enable
    await switchElement.click()
    await page.waitForTimeout(300)

    // Verify switch is now checked
    await expect(switchElement).toHaveAttribute('aria-checked', 'true')

    // Click switch again to disable
    await switchElement.click()
    await page.waitForTimeout(300)

    // Verify switch is back to unchecked
    await expect(switchElement).toHaveAttribute('aria-checked', 'false')

    // Close drawer
    await cancelGroupAssignment(page)

    // Cleanup
    await goToMcpAdminPage(page, baseURL)
    await deleteSystemServer(page, serverDisplayName)
    await goToUserGroupsPage(page, baseURL)
    await deleteUserGroup(page, groupName)
  })

  test('should show group description in drawer', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    const groupName = `test-group-desc-${Date.now()}`
    const groupDescription = 'This is a test group description'
    const serverName = `test-server-desc-${Date.now()}`
    const serverDisplayName = `Test Server Desc ${Date.now()}`

    await loginAsAdmin(page, baseURL)

    // Setup - create group with description
    await createUserGroup(page, baseURL, groupName, groupDescription)
    await createSystemServer(page, baseURL, serverName, serverDisplayName, 'Description test server')

    // Navigate to server detail page
    await goToMcpAdminPage(page, baseURL)
    await clickServerCard(page, serverDisplayName, true)

    // Open drawer
    await openGroupAssignmentDrawerFromServer(page, serverDisplayName)

    // Find the group card
    const groupCard = page.locator(
      `.ant-drawer:visible .ant-card:has(strong:has-text("${groupName}"))`
    )
    await expect(groupCard).toBeVisible()

    // Verify description is shown
    await expect(groupCard.locator(`text=${groupDescription}`)).toBeVisible()

    // Close drawer
    await cancelGroupAssignment(page)

    // Cleanup
    await goToMcpAdminPage(page, baseURL)
    await deleteSystemServer(page, serverDisplayName)
    await goToUserGroupsPage(page, baseURL)
    await deleteUserGroup(page, groupName)
  })
})
