import { Page, expect } from '@playwright/test'

/**
 * Helpers for managing MCP server <-> User group relationships
 */

// =====================================================
// User Group Navigation (reuse from LLM helpers)
// =====================================================

export async function goToUserGroupsPage(page: Page, baseURL: string) {
  await page.goto(`${baseURL}/settings/user-groups`)
  await page.waitForLoadState('load')
  // Wait for page to fully load before proceeding
  await waitForUserGroupsPageLoad(page)
}

export async function waitForUserGroupsPageLoad(page: Page) {
  // Wait for the page heading
  await page.waitForSelector('text=User Groups', { timeout: 30000 })
  // Wait for groups list to load
  await page.waitForLoadState('load')
  // Wait for content to render and API calls to complete
  await page.waitForTimeout(3000)
}

export async function clickGroupItem(page: Page, groupName: string) {
  // Note: Groups don't need to be "clicked" to expand - widgets are always visible
  // This function just waits for the group to be visible and scrolls it into view
  const groupText = page.locator(`text="${groupName}"`).first()
  await groupText.waitFor({ state: 'visible', timeout: 10000 })

  // Scroll the group into view to ensure widgets can render
  await groupText.scrollIntoViewIfNeeded()

  // Wait for lazy-loaded widgets to render
  await page.waitForTimeout(6000)
}

// =====================================================
// MCP Server Assignment in User Groups (Widget + Drawer)
// =====================================================

export async function openServerAssignmentDrawerFromGroup(
  page: Page,
  groupName: string
) {
  // First, expand the group if not already expanded
  await clickGroupItem(page, groupName)

  // Find the edit button with the specific aria-label
  const editButton = page.locator(`button[aria-label="Edit System MCP Servers for ${groupName}"]`)
  await editButton.waitFor({ state: 'visible', timeout: 10000 })
  await editButton.click()

  // Wait for drawer to open
  await page.waitForSelector('.ant-drawer-title:has-text("Assign System MCP Servers")', {
    state: 'visible',
    timeout: 5000,
  })
}

export async function toggleServerInDrawer(
  page: Page,
  serverName: string,
  enable: boolean
) {
  // Find the server card in the drawer by looking for the strong tag with the server name
  const serverCard = page.locator(
    `.ant-drawer.ant-drawer-open .ant-drawer-body .ant-card:has(strong:has-text("${serverName}"))`
  )
  await serverCard.waitFor({ state: 'visible', timeout: 5000 })

  // Get the switch state
  const switchElement = serverCard.locator('.ant-switch')
  const isChecked = (await switchElement.getAttribute('aria-checked')) === 'true'

  // Toggle if needed
  if (isChecked !== enable) {
    await switchElement.click()
    await page.waitForTimeout(300) // Wait for switch animation
  }
}

export async function saveServerAssignment(page: Page) {
  // Click Save button in drawer
  const saveButton = page.locator('.ant-drawer.ant-drawer-open button:has-text("Save")')
  await saveButton.click()

  // Wait for success message
  await page.waitForSelector('text=Server assignments updated', { timeout: 10000 })

  // Wait for drawer to close
  await page.waitForSelector('.ant-drawer-title:has-text("Assign System MCP Servers")', {
    state: 'hidden',
    timeout: 5000,
  })

  // Wait for event propagation and widget update
  // Need longer wait to allow store cache to invalidate and reload
  await page.waitForTimeout(2000)
}

export async function cancelServerAssignment(page: Page) {
  const cancelButton = page.locator('.ant-drawer.ant-drawer-open button:has-text("Cancel")')
  await cancelButton.click()

  // Wait for drawer to close
  await page.waitForSelector('.ant-drawer-title:has-text("Assign System MCP Servers")', {
    state: 'hidden',
    timeout: 5000,
  })
}

export async function assignServerToGroup(
  page: Page,
  groupName: string,
  serverName: string
) {
  // Ensure we're on the right page first
  await waitForUserGroupsPageLoad(page)
  await openServerAssignmentDrawerFromGroup(page, groupName)
  await toggleServerInDrawer(page, serverName, true)
  await saveServerAssignment(page)
}

export async function removeServerFromGroup(
  page: Page,
  groupName: string,
  serverName: string
) {
  // Ensure we're on the right page first
  await waitForUserGroupsPageLoad(page)
  await openServerAssignmentDrawerFromGroup(page, groupName)
  await toggleServerInDrawer(page, serverName, false)
  await saveServerAssignment(page)
}

export async function assertServerInGroupWidget(
  page: Page,
  groupName: string,
  serverName: string
) {
  // Ensure page has loaded
  await waitForUserGroupsPageLoad(page)

  // Expand the group if needed
  await clickGroupItem(page, groupName)

  // Find the specific widget by using the unique button as a locator
  const widget = page.locator(`[data-widget="system-mcp-servers"]:has(button[aria-label="Edit System MCP Servers for ${groupName}"])`).first()
  await widget.waitFor({ state: 'visible', timeout: 10000 })

  // Now find the server tag within this specific widget (no container)
  const serverTag = widget.locator(`.ant-tag:has-text("${serverName}")`)
  await expect(serverTag).toBeVisible()
}

export async function assertServerNotInGroupWidget(
  page: Page,
  groupName: string,
  serverName: string
) {
  // Ensure page has loaded
  await waitForUserGroupsPageLoad(page)

  // Expand the group if needed
  await clickGroupItem(page, groupName)

  // Find the specific widget
  const widget = page.locator(`[data-widget="system-mcp-servers"]:has(button[aria-label="Edit System MCP Servers for ${groupName}"])`).first()
  await widget.waitFor({ state: 'visible', timeout: 10000 })

  // Now find the server tag within this specific widget
  const serverTag = widget.locator(`.ant-tag:has-text("${serverName}")`)
  await expect(serverTag).not.toBeVisible()
}

export async function assertGroupWidgetShowsCount(
  page: Page,
  groupName: string,
  expectedCount: number
) {
  // Ensure page has loaded
  await waitForUserGroupsPageLoad(page)

  // Expand the group if needed
  await clickGroupItem(page, groupName)

  // Find the specific widget
  const widget = page.locator(`[data-widget="system-mcp-servers"]:has(button[aria-label="Edit System MCP Servers for ${groupName}"])`).first()
  await widget.waitFor({ state: 'visible', timeout: 10000 })

  if (expectedCount === 0) {
    // Look for "No servers assigned" text within this specific widget
    const noServersText = widget.locator('text=No servers assigned')
    await expect(noServersText).toBeVisible()
  } else {
    // Count tags within this specific widget only (no container)
    const tags = widget.locator('.ant-tag')
    await expect(tags).toHaveCount(expectedCount)
  }
}

// =====================================================
// Group Assignment in MCP Servers (Card + Drawer)
// =====================================================

/**
 * Locate the User Groups widget for a specific server. After the
 * Card→Collapse refactor (feat/mcp-rewrite-v2), the widget is identified by
 * `[data-card-type="user-groups-assignment"]` and scoped per-server via the
 * outer `[data-server-name]` Card wrapper.
 */
function groupsWidgetForServer(page: Page, serverDisplayName?: string) {
  if (serverDisplayName) {
    return page
      .locator(`[data-server-name="${serverDisplayName}"]`)
      .locator('[data-card-type="user-groups-assignment"]')
  }
  return page.locator('[data-card-type="user-groups-assignment"]').first()
}

/** Expand the Collapse so the assigned-groups list (or empty state) is visible. */
async function expandGroupsCollapseFor(page: Page, serverDisplayName?: string) {
  const widget = groupsWidgetForServer(page, serverDisplayName)
  await widget.waitFor({ state: 'visible', timeout: 10000 })
  const header = widget.locator('.ant-collapse-header').first()
  const expanded = (await header.getAttribute('aria-expanded')) === 'true'
  if (!expanded) {
    // Click the panel summary (the "User Groups" label). Avoid the edit
    // button's bounding box — it has `e.stopPropagation()`.
    await header.getByText('User Groups').first().click()
    await page.waitForTimeout(300)
  }
}

export async function openGroupAssignmentDrawerFromServer(
  page: Page,
  serverDisplayName?: string
) {
  const widget = groupsWidgetForServer(page, serverDisplayName)
  if (serverDisplayName) {
    await page
      .locator(`[data-server-name="${serverDisplayName}"]`)
      .scrollIntoViewIfNeeded()
    await page.waitForTimeout(300)
  }
  await widget.waitFor({ state: 'visible', timeout: 10000 })

  const editButton = widget.locator('button[aria-label="Manage user groups"]')
  await editButton.click()

  // Wait for drawer to open
  await page.waitForSelector('.ant-drawer-title:has-text("Assign User Groups")', {
    state: 'visible',
    timeout: 5000,
  })
}

export async function toggleGroupInDrawer(
  page: Page,
  groupName: string,
  enable: boolean
) {
  // Find the group card in the drawer
  const groupCard = page.locator(
    `.ant-drawer.ant-drawer-open .ant-card:has(strong:has-text("${groupName}"))`
  )
  await groupCard.waitFor({ state: 'visible', timeout: 5000 })

  // Get the switch state
  const switchElement = groupCard.locator('.ant-switch')
  const isChecked = (await switchElement.getAttribute('aria-checked')) === 'true'

  // Toggle if needed
  if (isChecked !== enable) {
    await switchElement.click()
    await page.waitForTimeout(300) // Wait for switch animation
  }
}

export async function saveGroupAssignment(page: Page) {
  // Click Save button in drawer
  const saveButton = page.locator('.ant-drawer.ant-drawer-open button:has-text("Save")')
  await saveButton.click()

  // Wait for success message
  await page.waitForSelector('text=Group assignments updated', { timeout: 10000 })

  // Wait for drawer to close
  await page.waitForSelector('.ant-drawer-title:has-text("Assign User Groups")', {
    state: 'hidden',
    timeout: 5000,
  })

  // Wait for event propagation and card update
  await page.waitForTimeout(1000)
}

export async function cancelGroupAssignment(page: Page) {
  const cancelButton = page.locator('.ant-drawer.ant-drawer-open button:has-text("Cancel")')
  await cancelButton.click()

  // Wait for drawer to close
  await page.waitForSelector('.ant-drawer-title:has-text("Assign User Groups")', {
    state: 'hidden',
    timeout: 5000,
  })
}

export async function assignGroupToServer(
  page: Page,
  _serverId: string,
  groupName: string,
  serverDisplayName?: string
) {
  await openGroupAssignmentDrawerFromServer(page, serverDisplayName)
  await toggleGroupInDrawer(page, groupName, true)
  await saveGroupAssignment(page)
}

export async function removeGroupFromServer(
  page: Page,
  _serverId: string,
  groupName: string,
  serverDisplayName?: string
) {
  await openGroupAssignmentDrawerFromServer(page, serverDisplayName)
  await toggleGroupInDrawer(page, groupName, false)
  await saveGroupAssignment(page)
}

export async function assertGroupInServerCard(
  page: Page,
  groupName: string,
  serverDisplayName?: string
) {
  await expandGroupsCollapseFor(page, serverDisplayName)
  const widget = groupsWidgetForServer(page, serverDisplayName)
  const groupTag = widget.locator(`.ant-tag:has-text("${groupName}")`)
  await expect(groupTag).toBeVisible()
}

export async function assertGroupNotInServerCard(
  page: Page,
  groupName: string,
  serverDisplayName?: string
) {
  await expandGroupsCollapseFor(page, serverDisplayName)
  const widget = groupsWidgetForServer(page, serverDisplayName)
  const groupTag = widget.locator(`.ant-tag:has-text("${groupName}")`)
  await expect(groupTag).not.toBeVisible()
}

export async function assertServerCardShowsCount(
  page: Page,
  expectedCount: number,
  serverDisplayName?: string
) {
  await expandGroupsCollapseFor(page, serverDisplayName)
  const widget = groupsWidgetForServer(page, serverDisplayName)

  if (expectedCount === 0) {
    await expect(
      widget.locator('.ant-empty-description:has-text("No groups assigned")'),
    ).toBeVisible()
  } else {
    const tags = widget.locator('.ant-tag')
    await expect(tags).toHaveCount(expectedCount)
  }
}

// =====================================================
// User Group Creation (for test setup)
// =====================================================

export async function createUserGroup(
  page: Page,
  baseURL: string,
  groupName: string,
  description?: string
): Promise<void> {
  await goToUserGroupsPage(page, baseURL)
  await waitForUserGroupsPageLoad(page)

  // Wait for and click Create group button
  const createButton = page.locator('button[aria-label="Create group"]')
  await createButton.waitFor({ state: 'visible', timeout: 10000 })
  await createButton.click()

  // Wait for drawer to open
  await page.waitForSelector('.ant-drawer-title:has-text("Create User Group")', {
    timeout: 5000,
  })

  // Fill form
  await page.fill('input[placeholder="Enter group name"]', groupName)
  if (description) {
    await page.fill('textarea[placeholder="Enter group description"]', description)
  }

  // Submit
  await page.click('.ant-drawer.ant-drawer-open button:has-text("Create Group")')

  // Wait for success message
  await page.waitForSelector('text=User group created successfully', { timeout: 10000 })

  // Wait for drawer to close
  await page.waitForSelector('.ant-drawer-title:has-text("Create User Group")', {
    state: 'hidden',
    timeout: 5000,
  })

  // Verify group appears in list
  await expect(page.locator(`text=${groupName}`).first()).toBeVisible()
}

export async function deleteUserGroup(
  page: Page,
  groupName: string
): Promise<void> {
  // Wait for the group to be visible
  await clickGroupItem(page, groupName)

  // Click delete button using aria-label
  const deleteButton = page.locator(`button[aria-label="Delete ${groupName}"]`).first()
  await deleteButton.waitFor({ state: 'visible', timeout: 10000 })
  await deleteButton.click()

  // Confirm deletion
  await page.waitForSelector('.ant-popconfirm', { state: 'visible', timeout: 5000 })
  await page.click('.ant-popconfirm button:has-text("Yes")')

  // Wait for success message
  await page.waitForSelector('text=User group deleted successfully', { timeout: 10000 })

  // Verify group is gone
  await expect(page.locator(`text="${groupName}"`).first()).not.toBeVisible()
}
