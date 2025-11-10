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
  // Wait for page to fully load before proceeding
  await waitForUserGroupsPageLoad(page)
}

export async function waitForUserGroupsPageLoad(page: Page) {
  // Wait for the page heading
  await page.waitForSelector('text=User Groups', { timeout: 30000 })
  // Wait for groups list to load - use 'load' not 'networkidle' to avoid SSE issues
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

  // Wait for lazy-loaded widgets to render (LLM Providers widget is lazy-loaded)
  // Increased to 6 seconds to give more time for lazy component loading and API calls
  await page.waitForTimeout(6000)
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
  // Find the provider card in the drawer by looking for the strong tag with the provider name
  // The structure is: Card > div > (switch div + content div) > div > strong
  const providerCard = page.locator(
    `.ant-drawer:visible .ant-drawer-body .ant-card:has(strong:has-text("${providerName}"))`
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

  // Wait for event propagation and widget update
  // The widget now loads data on mount AND listens to events for updates
  await page.waitForTimeout(1000)
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
  // Ensure we're on the right page first
  await waitForUserGroupsPageLoad(page)
  await openProviderAssignmentDrawerFromGroup(page, groupName)
  await toggleProviderInDrawer(page, providerName, true)
  await saveProviderAssignment(page)
}

export async function removeProviderFromGroup(
  page: Page,
  groupName: string,
  providerName: string
) {
  // Ensure we're on the right page first
  await waitForUserGroupsPageLoad(page)
  await openProviderAssignmentDrawerFromGroup(page, groupName)
  await toggleProviderInDrawer(page, providerName, false)
  await saveProviderAssignment(page)
}

export async function assertProviderInGroupWidget(
  page: Page,
  groupName: string,
  providerName: string
) {
  // Ensure page has loaded
  await waitForUserGroupsPageLoad(page)

  // Expand the group if needed
  await clickGroupItem(page, groupName)

  // Find the specific widget by using the unique button as a locator
  // The widget contains the button, so we find the widget that has this specific button
  // Use .first() to handle potential duplicate widgets
  const widget = page.locator(`[data-widget="llm-providers"]:has(button[aria-label="Edit LLM Providers for ${groupName}"])`).first()
  await widget.waitFor({ state: 'visible', timeout: 10000 })

  // Now find the provider tag within this specific widget
  const providerTag = widget.locator(`[data-testid="provider-tags-container"] .ant-tag:has-text("${providerName}")`)
  await expect(providerTag).toBeVisible()
}

export async function assertProviderNotInGroupWidget(
  page: Page,
  groupName: string,
  providerName: string
) {
  // Ensure page has loaded
  await waitForUserGroupsPageLoad(page)

  // Expand the group if needed
  await clickGroupItem(page, groupName)

  // Find the specific widget by using the unique button as a locator
  // Use .first() to handle potential duplicate widgets
  const widget = page.locator(`[data-widget="llm-providers"]:has(button[aria-label="Edit LLM Providers for ${groupName}"])`).first()
  await widget.waitFor({ state: 'visible', timeout: 10000 })

  // Now find the provider tag within this specific widget
  const providerTag = widget.locator(`[data-testid="provider-tags-container"] .ant-tag:has-text("${providerName}")`)
  await expect(providerTag).not.toBeVisible()
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

  // Find the specific widget by using the unique button as a locator
  // This ensures we're looking at the right widget even when multiple groups are on the page
  // Use .first() to handle potential duplicate widgets
  const widget = page.locator(`[data-widget="llm-providers"]:has(button[aria-label="Edit LLM Providers for ${groupName}"])`).first()
  await widget.waitFor({ state: 'visible', timeout: 10000 })

  if (expectedCount === 0) {
    // Look for "No providers assigned" text within this specific widget
    const noProvidersText = widget.locator('text=No providers assigned')
    await expect(noProvidersText).toBeVisible()
  } else {
    // Count tags within this specific widget only
    const tags = widget.locator('[data-testid="provider-tags-container"] .ant-tag')
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
  // Find the group item in the drawer by looking for the strong tag with the group name
  // The structure is: drawer > body > generic container > switch + text container > strong
  const groupContainer = page.locator(
    `.ant-drawer:visible .ant-drawer-body > div > div:has(strong:has-text("${groupName}"))`
  )
  await groupContainer.waitFor({ state: 'visible', timeout: 5000 })

  // Get the switch state
  const switchElement = groupContainer.locator('.ant-switch')
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

  // Wait for event propagation and card update
  // The card now loads data on mount AND listens to events for updates
  await page.waitForTimeout(1000)
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
  await expect(page.locator(`text=${groupName}`).first()).toBeVisible()
}

export async function deleteUserGroup(
  page: Page,
  groupName: string
): Promise<void> {
  // Wait for the group to be visible
  await clickGroupItem(page, groupName)

  // Click delete button using aria-label
  // Use .first() to handle potential duplicates
  const deleteButton = page.locator(`button[aria-label="Delete ${groupName}"]`).first()
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
