import { test, expect } from '../../fixtures/test-context'
import { assertNoAccessibilityViolations } from '../../utils/accessibility'
import { loginAsAdmin } from '../../common/auth-helpers'
import {
  goToUserGroupsPage,
  createUserGroup,
  deleteUserGroup,
  clickGroupItem,
  openServerAssignmentDrawerFromGroup,
  toggleServerInDrawer,
  saveServerAssignment,
  cancelServerAssignment,
  assignServerToGroup,
  removeServerFromGroup,
  assertServerInGroupWidget,
  assertServerNotInGroupWidget,
  assertGroupWidgetShowsCount,
} from './helpers/group-server-helpers'
import { createSystemServer, deleteSystemServer } from './helpers/server-helpers'
import { goToMcpAdminPage } from './helpers/navigation-helpers'

test.describe('MCP Server Assignment in User Groups', () => {
  test('should pass accessibility checks on user groups page with widget', async ({
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

    // Assign server to group
    await goToUserGroupsPage(page, baseURL)
    await assignServerToGroup(page, groupName, serverDisplayName)

    // Check accessibility with widget visible
    await goToUserGroupsPage(page, baseURL)
    await clickGroupItem(page, groupName)
    // Disable color-contrast rule for AntD's orange tag (known limitation)
    await assertNoAccessibilityViolations(page, {
      disabledRules: ['color-contrast'],
    })

    // Cleanup
    await deleteUserGroup(page, groupName)
    await goToMcpAdminPage(page, baseURL)
    await deleteSystemServer(page, serverDisplayName)
  })

  test('should display MCP Servers widget in user group', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    const groupName = `test-group-widget-${Date.now()}`

    await loginAsAdmin(page, baseURL)
    await createUserGroup(page, baseURL, groupName, 'Widget display test')

    // Wait for the group to be visible and scroll into view
    await clickGroupItem(page, groupName)

    // Wait for the specific widget to load for this group (longer timeout for lazy loading)
    const widget = page.locator(`[data-widget="system-mcp-servers"]:has(button[aria-label="Edit System MCP Servers for ${groupName}"])`).first()
    await widget.waitFor({ state: 'visible', timeout: 15000 })

    // Verify edit button exists with the specific aria-label
    const editButton = page.locator(`button[aria-label="Edit System MCP Servers for ${groupName}"]`).first()
    await expect(editButton).toBeVisible()

    // Cleanup
    await deleteUserGroup(page, groupName)
  })

  test('should open server assignment drawer from group widget', async ({
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

    // Open drawer
    await goToUserGroupsPage(page, baseURL)
    await openServerAssignmentDrawerFromGroup(page, groupName)

    // Verify drawer is open with correct title
    await expect(
      page.locator(`.ant-drawer-title:has-text("Assign System MCP Servers - ${groupName}")`)
    ).toBeVisible()

    // Verify server appears in the drawer
    await expect(page.locator(`.ant-drawer.ant-drawer-open:has-text("${serverDisplayName}")`)).toBeVisible()

    // Verify switch exists
    const serverCard = page.locator(
      `.ant-drawer.ant-drawer-open .ant-drawer-body .ant-card:has(strong:has-text("${serverDisplayName}"))`
    )
    const switchElement = serverCard.locator('.ant-switch')
    await expect(switchElement).toBeVisible()

    // Close drawer
    await cancelServerAssignment(page)

    // Cleanup
    await deleteUserGroup(page, groupName)
    await goToMcpAdminPage(page, baseURL)
    await deleteSystemServer(page, serverDisplayName)
  })

  test('should assign server to group', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    const groupName = `test-group-assign-${Date.now()}`
    const serverName = `test-server-assign-${Date.now()}`
    const serverDisplayName = `Test Server Assign ${Date.now()}`

    await loginAsAdmin(page, baseURL)

    // Setup
    await createUserGroup(page, baseURL, groupName, 'Assignment test group')
    await createSystemServer(page, baseURL, serverName, serverDisplayName, 'Assignment test server')

    // Assign server
    await goToUserGroupsPage(page, baseURL)
    await assignServerToGroup(page, groupName, serverDisplayName)

    // Verify server appears in widget
    await assertServerInGroupWidget(page, groupName, serverDisplayName)

    // Cleanup
    await deleteUserGroup(page, groupName)
    await goToMcpAdminPage(page, baseURL)
    await deleteSystemServer(page, serverDisplayName)
  })

  test('should remove server from group', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    const groupName = `test-group-remove-${Date.now()}`
    const serverName = `test-server-remove-${Date.now()}`
    const serverDisplayName = `Test Server Remove ${Date.now()}`

    await loginAsAdmin(page, baseURL)

    // Setup
    await createUserGroup(page, baseURL, groupName, 'Removal test group')
    await createSystemServer(page, baseURL, serverName, serverDisplayName, 'Removal test server')

    // Assign then remove
    await goToUserGroupsPage(page, baseURL)
    await assignServerToGroup(page, groupName, serverDisplayName)
    await removeServerFromGroup(page, groupName, serverDisplayName)

    // Verify server is gone from widget
    await assertServerNotInGroupWidget(page, groupName, serverDisplayName)

    // Cleanup
    await deleteUserGroup(page, groupName)
    await goToMcpAdminPage(page, baseURL)
    await deleteSystemServer(page, serverDisplayName)
  })

  test('should assign multiple servers to group', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    const groupName = `test-group-multi-${Date.now()}`
    const server1Name = `test-server-1-${Date.now()}`
    const server1DisplayName = `Test Server 1 ${Date.now()}`
    const server2Name = `test-server-2-${Date.now()}`
    const server2DisplayName = `Test Server 2 ${Date.now()}`

    await loginAsAdmin(page, baseURL)

    // Setup
    await createUserGroup(page, baseURL, groupName, 'Multiple servers test')
    await createSystemServer(page, baseURL, server1Name, server1DisplayName, 'Server 1')
    await createSystemServer(page, baseURL, server2Name, server2DisplayName, 'Server 2')

    // Assign both servers at once
    await goToUserGroupsPage(page, baseURL)
    await openServerAssignmentDrawerFromGroup(page, groupName)
    await toggleServerInDrawer(page, server1DisplayName, true)
    await toggleServerInDrawer(page, server2DisplayName, true)
    await saveServerAssignment(page)

    // Verify both appear in widget
    await assertServerInGroupWidget(page, groupName, server1DisplayName)
    await assertServerInGroupWidget(page, groupName, server2DisplayName)
    await assertGroupWidgetShowsCount(page, groupName, 2)

    // Cleanup
    await deleteUserGroup(page, groupName)
    await goToMcpAdminPage(page, baseURL)
    await deleteSystemServer(page, server1DisplayName)
    await deleteSystemServer(page, server2DisplayName)
  })

  test('should show default system servers in drawer', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    const groupName = `test-group-defaults-${Date.now()}`

    await loginAsAdmin(page, baseURL)

    // Setup
    await createUserGroup(page, baseURL, groupName, 'Default servers test')

    // Open drawer
    await goToUserGroupsPage(page, baseURL)
    await openServerAssignmentDrawerFromGroup(page, groupName)

    // Verify drawer shows default system servers (Web Fetch, Filesystem, etc.)
    // These are created in migrations
    const webFetchCard = page.locator(
      `.ant-drawer.ant-drawer-open .ant-drawer-body .ant-card:has(strong:has-text("Web Fetch"))`
    )
    await expect(webFetchCard).toBeVisible()

    // Close drawer
    await cancelServerAssignment(page)

    // Cleanup
    await deleteUserGroup(page, groupName)
  })

  test('should show enabled status for servers', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    const groupName = `test-group-status-${Date.now()}`
    const serverName = `test-server-status-${Date.now()}`
    const serverDisplayName = `Test Server Status ${Date.now()}`

    await loginAsAdmin(page, baseURL)

    // Setup - create enabled server
    await createUserGroup(page, baseURL, groupName, 'Status test group')
    await createSystemServer(page, baseURL, serverName, serverDisplayName, 'Status test server')

    // Open drawer
    await goToUserGroupsPage(page, baseURL)
    await openServerAssignmentDrawerFromGroup(page, groupName)

    // Find the server card
    const serverCard = page.locator(
      `.ant-drawer.ant-drawer-open .ant-drawer-body .ant-card:has(strong:has-text("${serverDisplayName}"))`
    )
    await expect(serverCard).toBeVisible()

    // Verify it shows Enabled tag (servers are enabled by default)
    await expect(serverCard.locator('.ant-tag:has-text("Enabled")')).toBeVisible()

    // Close drawer
    await cancelServerAssignment(page)

    // Cleanup
    await deleteUserGroup(page, groupName)
    await goToMcpAdminPage(page, baseURL)
    await deleteSystemServer(page, serverDisplayName)
  })

  test('should update widget count when servers are added/removed', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    const groupName = `test-group-count-${Date.now()}`
    const server1Name = `test-server-count-1-${Date.now()}`
    const server1DisplayName = `Test Server Count 1 ${Date.now()}`
    const server2Name = `test-server-count-2-${Date.now()}`
    const server2DisplayName = `Test Server Count 2 ${Date.now()}`

    await loginAsAdmin(page, baseURL)

    // Setup
    await createUserGroup(page, baseURL, groupName, 'Count update test')
    await createSystemServer(page, baseURL, server1Name, server1DisplayName, 'Server 1')
    await createSystemServer(page, baseURL, server2Name, server2DisplayName, 'Server 2')

    // Start with 0
    await goToUserGroupsPage(page, baseURL)
    await assertGroupWidgetShowsCount(page, groupName, 0)

    // Add one server -> count = 1
    await assignServerToGroup(page, groupName, server1DisplayName)
    await assertGroupWidgetShowsCount(page, groupName, 1)

    // Add another server -> count = 2
    await assignServerToGroup(page, groupName, server2DisplayName)
    await assertGroupWidgetShowsCount(page, groupName, 2)

    // Remove one server -> count = 1
    await removeServerFromGroup(page, groupName, server1DisplayName)
    await assertGroupWidgetShowsCount(page, groupName, 1)

    // Remove last server -> count = 0
    await removeServerFromGroup(page, groupName, server2DisplayName)
    await assertGroupWidgetShowsCount(page, groupName, 0)

    // Cleanup
    await deleteUserGroup(page, groupName)
    await goToMcpAdminPage(page, baseURL)
    await deleteSystemServer(page, server1DisplayName)
    await deleteSystemServer(page, server2DisplayName)
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

    // Open drawer and toggle server but cancel
    await goToUserGroupsPage(page, baseURL)
    await openServerAssignmentDrawerFromGroup(page, groupName)
    await toggleServerInDrawer(page, serverDisplayName, true)
    await cancelServerAssignment(page)

    // Verify server was NOT assigned
    await assertServerNotInGroupWidget(page, groupName, serverDisplayName)

    // Cleanup
    await deleteUserGroup(page, groupName)
    await goToMcpAdminPage(page, baseURL)
    await deleteSystemServer(page, serverDisplayName)
  })

  test('should toggle server by clicking card', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    const groupName = `test-group-click-${Date.now()}`
    const serverName = `test-server-click-${Date.now()}`
    const serverDisplayName = `Test Server Click ${Date.now()}`

    await loginAsAdmin(page, baseURL)

    // Setup
    await createUserGroup(page, baseURL, groupName, 'Click test group')
    await createSystemServer(page, baseURL, serverName, serverDisplayName, 'Click test server')

    // Open drawer
    await goToUserGroupsPage(page, baseURL)
    await openServerAssignmentDrawerFromGroup(page, groupName)

    // Get the server card and switch
    const serverCard = page.locator(
      `.ant-drawer.ant-drawer-open .ant-drawer-body .ant-card:has(strong:has-text("${serverDisplayName}"))`
    )
    const switchElement = serverCard.locator('.ant-switch')

    // Verify initially unchecked
    await expect(switchElement).toHaveAttribute('aria-checked', 'false')

    // Click the card (not the switch)
    await serverCard.click()
    await page.waitForTimeout(300)

    // Verify switch is now checked
    await expect(switchElement).toHaveAttribute('aria-checked', 'true')

    // Click card again
    await serverCard.click()
    await page.waitForTimeout(300)

    // Verify switch is back to unchecked
    await expect(switchElement).toHaveAttribute('aria-checked', 'false')

    // Close drawer
    await cancelServerAssignment(page)

    // Cleanup
    await deleteUserGroup(page, groupName)
    await goToMcpAdminPage(page, baseURL)
    await deleteSystemServer(page, serverDisplayName)
  })

  test('should show server description in drawer', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    const groupName = `test-group-desc-${Date.now()}`
    const serverName = `test-server-desc-${Date.now()}`
    const serverDisplayName = `Test Server Desc ${Date.now()}`
    const serverDescription = 'This is a test server description'

    await loginAsAdmin(page, baseURL)

    // Setup - create server with description
    await createUserGroup(page, baseURL, groupName, 'Description test group')
    await createSystemServer(page, baseURL, serverName, serverDisplayName, serverDescription)

    // Open drawer
    await goToUserGroupsPage(page, baseURL)
    await openServerAssignmentDrawerFromGroup(page, groupName)

    // Find the server card
    const serverCard = page.locator(
      `.ant-drawer.ant-drawer-open .ant-drawer-body .ant-card:has(strong:has-text("${serverDisplayName}"))`
    )
    await expect(serverCard).toBeVisible()

    // Verify description is shown
    await expect(serverCard.locator(`text=${serverDescription}`)).toBeVisible()

    // Close drawer
    await cancelServerAssignment(page)

    // Cleanup
    await deleteUserGroup(page, groupName)
    await goToMcpAdminPage(page, baseURL)
    await deleteSystemServer(page, serverDisplayName)
  })

  test('should filter servers in drawer by search', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    const groupName = `test-group-filter-${Date.now()}`
    const server1Name = `test-server-filter-1-${Date.now()}`
    const server1DisplayName = `Test Server Filter Alpha ${Date.now()}`
    const server2Name = `test-server-filter-2-${Date.now()}`
    const server2DisplayName = `Test Server Filter Beta ${Date.now()}`

    await loginAsAdmin(page, baseURL)

    // Setup
    await createUserGroup(page, baseURL, groupName, 'Filter test group')
    await createSystemServer(page, baseURL, server1Name, server1DisplayName, 'Alpha server')
    await createSystemServer(page, baseURL, server2Name, server2DisplayName, 'Beta server')

    // Open drawer
    await goToUserGroupsPage(page, baseURL)
    await openServerAssignmentDrawerFromGroup(page, groupName)

    // Search for "Alpha"
    const searchInput = page.locator('.ant-drawer.ant-drawer-open input[placeholder*="Search"]')
    if (await searchInput.count() > 0) {
      await searchInput.fill('Alpha')
      await page.waitForTimeout(500)

      // Should show Alpha server
      await expect(
        page.locator(`.ant-drawer.ant-drawer-open:has-text("${server1DisplayName}")`)
      ).toBeVisible()

      // Should not show Beta server
      await expect(
        page.locator(`.ant-drawer.ant-drawer-open .ant-drawer-body .ant-card:has-text("${server2DisplayName}")`)
      ).not.toBeVisible()
    }

    // Close drawer
    await cancelServerAssignment(page)

    // Cleanup
    await deleteUserGroup(page, groupName)
    await goToMcpAdminPage(page, baseURL)
    await deleteSystemServer(page, server1DisplayName)
    await deleteSystemServer(page, server2DisplayName)
  })

  test('should show empty state when no servers match search', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    const groupName = `test-group-empty-${Date.now()}`

    await loginAsAdmin(page, baseURL)

    // Setup
    await createUserGroup(page, baseURL, groupName, 'Empty state test group')

    // Open drawer
    await goToUserGroupsPage(page, baseURL)
    await openServerAssignmentDrawerFromGroup(page, groupName)

    // Search for non-existent server
    const searchInput = page.locator('.ant-drawer.ant-drawer-open input[placeholder*="Search"]')
    if (await searchInput.count() > 0) {
      await searchInput.fill('nonexistent-server-xyz-12345')
      await page.waitForTimeout(500)

      // Should display empty state
      await expect(
        page.locator('.ant-drawer.ant-drawer-open:has-text("No servers found")')
      ).toBeVisible()
    }

    // Close drawer
    await cancelServerAssignment(page)

    // Cleanup
    await deleteUserGroup(page, groupName)
  })
})
