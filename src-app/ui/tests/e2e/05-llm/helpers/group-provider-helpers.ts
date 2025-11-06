import { Page, expect } from '@playwright/test'

/**
 * Helpers for managing LLM provider <-> User group relationships
 */

// =====================================================
// User Group Navigation
// =====================================================

export async function goToUserGroupsPage(page: Page, baseURL: string) {
  await page.goto(`${baseURL}/settings/user-groups`)
  await page.waitForLoadState('load')
}

export async function waitForUserGroupsPageLoad(page: Page) {
  // Wait for the page heading
  await page.waitForSelector('text=User Groups', { timeout: 30000 })
  // Wait for groups list to load - use 'load' not 'networkidle' to avoid SSE issues
  await page.waitForLoadState('load')
  // Wait for content to render
  await page.waitForTimeout(1000)
}

export async function clickGroupItem(page: Page, groupName: string) {
  // Note: Groups don't need to be "clicked" to expand - widgets are always visible
  // This function just waits for the group to be visible in the page
  const groupText = page.locator(`text="${groupName}"`).first()
  await groupText.waitFor({ state: 'visible', timeout: 10000 })
  // No need to click - widgets are already visible
  await page.waitForTimeout(500) // Wait for any rendering
}

// =====================================================
// Provider Assignment in User Groups (Widget + Drawer)
// =====================================================

export async function openProviderAssignmentDrawerFromGroup(
  page: Page,
  groupName: string
) {
  // First, expand the group if not already expanded
  await clickGroupItem(page, groupName)

  // Find the edit button with the specific aria-label
  const editButton = page.locator(`button[aria-label="Edit LLM Providers for ${groupName}"]`)
  await editButton.waitFor({ state: 'visible', timeout: 10000 })
  await editButton.click()

  // Wait for drawer to open
  await page.waitForSelector('.ant-drawer-title:has-text("Assign LLM Providers")', {
    state: 'visible',
    timeout: 5000,
  })
}

export async function toggleProviderInDrawer(
  page: Page,
  providerName: string,
  enable: boolean
) {
  // Find the provider card in the drawer
  const providerCard = page.locator(
    `.ant-drawer:visible .ant-drawer-body .p-3:has-text("${providerName}")`
  )
  await providerCard.waitFor({ state: 'visible', timeout: 5000 })

  // Get the switch state
  const switchElement = providerCard.locator('.ant-switch')
  const isChecked = (await switchElement.getAttribute('aria-checked')) === 'true'

  // Toggle if needed
  if (isChecked !== enable) {
    await switchElement.click()
    await page.waitForTimeout(300) // Wait for switch animation
  }
}

export async function saveProviderAssignment(page: Page) {
  // Click Save button in drawer
  const saveButton = page.locator('.ant-drawer:visible button:has-text("Save")')
  await saveButton.click()

  // Wait for success message
  await page.waitForSelector('text=Provider assignments updated', { timeout: 10000 })

  // Wait for drawer to close
  await page.waitForSelector('.ant-drawer-title:has-text("Assign LLM Providers")', {
    state: 'hidden',
    timeout: 5000,
  })
}

export async function cancelProviderAssignment(page: Page) {
  const cancelButton = page.locator('.ant-drawer:visible button:has-text("Cancel")')
  await cancelButton.click()

  // Wait for drawer to close
  await page.waitForSelector('.ant-drawer-title:has-text("Assign LLM Providers")', {
    state: 'hidden',
    timeout: 5000,
  })
}

export async function assignProviderToGroup(
  page: Page,
  groupName: string,
  providerName: string
) {
  await openProviderAssignmentDrawerFromGroup(page, groupName)
  await toggleProviderInDrawer(page, providerName, true)
  await saveProviderAssignment(page)
}

export async function removeProviderFromGroup(
  page: Page,
  groupName: string,
  providerName: string
) {
  await openProviderAssignmentDrawerFromGroup(page, groupName)
  await toggleProviderInDrawer(page, providerName, false)
  await saveProviderAssignment(page)
}

export async function assertProviderInGroupWidget(
  page: Page,
  groupName: string,
  providerName: string
) {
  // Expand the group if needed
  await clickGroupItem(page, groupName)

  // Find the group container that contains the group name
  // Then find the LLM Providers widget within that group
  const groupContainer = page.locator(`[role="listitem"]:has-text("${groupName}")`).first()
  const widget = groupContainer.locator('div:has(strong:has-text("LLM Providers"))')
  await widget.waitFor({ state: 'visible', timeout: 5000 })

  const providerTag = widget.locator(`.ant-tag:has-text("${providerName}")`)
  await expect(providerTag).toBeVisible()
}

export async function assertProviderNotInGroupWidget(
  page: Page,
  groupName: string,
  providerName: string
) {
  // Expand the group if needed
  await clickGroupItem(page, groupName)

  // Find the group container that contains the group name
  // Then find the LLM Providers widget within that group
  const groupContainer = page.locator(`[role="listitem"]:has-text("${groupName}")`).first()
  const widget = groupContainer.locator('div:has(strong:has-text("LLM Providers"))')
  await widget.waitFor({ state: 'visible', timeout: 5000 })

  const providerTag = widget.locator(`.ant-tag:has-text("${providerName}")`)
  await expect(providerTag).not.toBeVisible()
}

export async function assertGroupWidgetShowsCount(
  page: Page,
  groupName: string,
  expectedCount: number
) {
  // Expand the group if needed
  await clickGroupItem(page, groupName)

  // Find the group container that contains the group name
  // Then find the LLM Providers widget within that group
  const groupContainer = page.locator(`[role="listitem"]:has-text("${groupName}")`).first()
  const widget = groupContainer.locator('div:has(strong:has-text("LLM Providers"))')
  await widget.waitFor({ state: 'visible', timeout: 5000 })

  if (expectedCount === 0) {
    // Check for "No providers assigned" text
    await expect(widget.locator('text=No providers assigned')).toBeVisible()
  } else {
    const tags = widget.locator('.ant-tag')
    await expect(tags).toHaveCount(expectedCount)
  }
}

// =====================================================
// Group Assignment in Providers (Card + Drawer)
// =====================================================

export async function openGroupAssignmentDrawerFromProvider(
  page: Page
) {
  // Should already be on provider detail page
  // Find the User Groups card and click the edit button
  const card = page.locator('.ant-card:has(.ant-card-head-title:has-text("User Groups"))')
  await card.waitFor({ state: 'visible', timeout: 10000 })

  const editButton = card.locator('button[aria-label="Manage user groups"]')
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
    `.ant-drawer:visible .ant-drawer-body .p-3:has-text("${groupName}")`
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
  const saveButton = page.locator('.ant-drawer:visible button:has-text("Save")')
  await saveButton.click()

  // Wait for success message
  await page.waitForSelector('text=Group assignments updated', { timeout: 10000 })

  // Wait for drawer to close
  await page.waitForSelector('.ant-drawer-title:has-text("Assign User Groups")', {
    state: 'hidden',
    timeout: 5000,
  })
}

export async function cancelGroupAssignment(page: Page) {
  const cancelButton = page.locator('.ant-drawer:visible button:has-text("Cancel")')
  await cancelButton.click()

  // Wait for drawer to close
  await page.waitForSelector('.ant-drawer-title:has-text("Assign User Groups")', {
    state: 'hidden',
    timeout: 5000,
  })
}

export async function assignGroupToProvider(
  page: Page,
  _providerId: string,
  groupName: string
) {
  await openGroupAssignmentDrawerFromProvider(page)
  await toggleGroupInDrawer(page, groupName, true)
  await saveGroupAssignment(page)
}

export async function removeGroupFromProvider(
  page: Page,
  _providerId: string,
  groupName: string
) {
  await openGroupAssignmentDrawerFromProvider(page)
  await toggleGroupInDrawer(page, groupName, false)
  await saveGroupAssignment(page)
}

export async function assertGroupInProviderCard(
  page: Page,
  groupName: string
) {
  // Find the card and verify group tag exists
  const card = page.locator('.ant-card:has(.ant-card-head-title:has-text("User Groups"))')
  await card.waitFor({ state: 'visible', timeout: 5000 })

  const groupTag = card.locator(`.ant-tag:has-text("${groupName}")`)
  await expect(groupTag).toBeVisible()
}

export async function assertGroupNotInProviderCard(
  page: Page,
  groupName: string
) {
  // Find the card
  const card = page.locator('.ant-card:has(.ant-card-head-title:has-text("User Groups"))')
  await card.waitFor({ state: 'visible', timeout: 5000 })

  const groupTag = card.locator(`.ant-tag:has-text("${groupName}")`)
  await expect(groupTag).not.toBeVisible()
}

export async function assertProviderCardShowsCount(
  page: Page,
  expectedCount: number
) {
  // Find the card and count tags
  const card = page.locator('.ant-card:has(.ant-card-head-title:has-text("User Groups"))')
  await card.waitFor({ state: 'visible', timeout: 5000 })

  if (expectedCount === 0) {
    // Check for empty state
    await expect(card.locator('text=No groups assigned')).toBeVisible()
  } else {
    const tags = card.locator('.ant-tag')
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

  // Wait for and click Create group button (it's an icon-only button with aria-label)
  const createButton = page.locator('button[aria-label="Create group"]')
  await createButton.waitFor({ state: 'visible', timeout: 10000 })
  await createButton.click()

  // Wait for drawer to open
  await page.waitForSelector('.ant-drawer-title:has-text("Create User Group")', {
    timeout: 5000,
  })

  // Fill form using placeholders (form has no ID)
  await page.fill('input[placeholder="Enter group name"]', groupName)
  if (description) {
    await page.fill('textarea[placeholder="Enter group description"]', description)
  }

  // Submit - button text is "Create Group", not "Create"
  await page.click('.ant-drawer:visible button:has-text("Create Group")')

  // Wait for success message
  await page.waitForSelector('text=User group created successfully', { timeout: 10000 })

  // Wait for drawer to close
  await page.waitForSelector('.ant-drawer-title:has-text("Create User Group")', {
    state: 'hidden',
    timeout: 5000,
  })

  // Verify group appears in list
  await expect(page.locator(`text=${groupName}`)).toBeVisible()
}

export async function deleteUserGroup(
  page: Page,
  groupName: string
): Promise<void> {
  // Wait for the group to be visible
  await clickGroupItem(page, groupName)

  // Click delete button using aria-label
  const deleteButton = page.locator(`button[aria-label="Delete ${groupName}"]`)
  await deleteButton.waitFor({ state: 'visible', timeout: 10000 })
  await deleteButton.click()

  // Confirm deletion - look for "Yes" button in confirmation dialog
  await page.waitForSelector('.ant-popconfirm', { state: 'visible', timeout: 5000 })
  await page.click('.ant-popconfirm button:has-text("Yes")')

  // Wait for success message
  await page.waitForSelector('text=User group deleted successfully', { timeout: 10000 })

  // Verify group is gone
  await expect(page.locator(`text="${groupName}"`).first()).not.toBeVisible()
}
