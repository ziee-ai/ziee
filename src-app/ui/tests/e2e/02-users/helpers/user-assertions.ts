import { Page, expect } from '@playwright/test'

/**
 * Assert that a user exists in the list
 */
export async function assertUserExists(page: Page, username: string) {
  // Find username in the user info section (more specific than just text)
  const userElement = page.locator('.ant-typography.font-medium', { hasText: username })
  await expect(userElement.first()).toBeVisible({ timeout: 5000 })
}

/**
 * Assert that a user does not exist in the list
 */
export async function assertUserNotExists(page: Page, username: string) {
  // Find username in the user info section (more specific than just text)
  const userElement = page.locator('.ant-typography.font-medium', { hasText: username })
  await expect(userElement).not.toBeVisible({ timeout: 5000 })
}

/**
 * Assert user status (active/inactive)
 */
export async function assertUserStatus(
  page: Page,
  username: string,
  expectedStatus: 'active' | 'inactive'
) {
  const userRow = page.locator(`text="${username}"`).locator('..')
  const statusBadge = userRow.locator('.ant-badge-status-text')

  const statusText = await statusBadge.textContent()
  expect(statusText?.toLowerCase()).toContain(expectedStatus)
}

/**
 * Assert user email
 */
export async function assertUserEmail(page: Page, username: string, expectedEmail: string) {
  const userRow = page.locator(`text="${username}"`).locator('..')
  const emailElement = userRow.locator(`text="${expectedEmail}"`)
  await expect(emailElement).toBeVisible()
}

/**
 * Assert that a group exists in the list
 */
export async function assertGroupExists(page: Page, groupName: string) {
  const groupElement = page.locator('text=' + groupName)
  await expect(groupElement).toBeVisible({ timeout: 5000 })
}

/**
 * Assert that a group does not exist in the list
 */
export async function assertGroupNotExists(page: Page, groupName: string) {
  const groupElement = page.locator('text=' + groupName)
  await expect(groupElement).not.toBeVisible({ timeout: 5000 })
}

/**
 * Assert group status (active/inactive)
 */
export async function assertGroupStatus(
  page: Page,
  groupName: string,
  expectedStatus: 'active' | 'inactive'
) {
  const groupCard = page.locator('.ant-card', { hasText: groupName })
  const statusBadge = groupCard.locator('.ant-badge-status-text')

  const statusText = await statusBadge.textContent()
  expect(statusText?.toLowerCase()).toContain(expectedStatus)
}

/**
 * Assert group has system tag
 */
export async function assertGroupIsSystem(page: Page, groupName: string) {
  const groupCard = page.locator('.ant-card', { hasText: groupName })
  const systemTag = groupCard.locator('.ant-tag', { hasText: /system/i })
  await expect(systemTag).toBeVisible()
}

/**
 * Assert user is in group members list
 */
export async function assertUserInGroup(page: Page, username: string) {
  const drawer = page.locator('.ant-drawer:visible')
  const userItem = drawer.locator('.ant-list-item', { hasText: username })
  await expect(userItem).toBeVisible({ timeout: 5000 })
}

/**
 * Assert user is not in group members list
 */
export async function assertUserNotInGroup(page: Page, username: string) {
  const drawer = page.locator('.ant-drawer:visible')
  const userItem = drawer.locator('.ant-list-item', { hasText: username })
  await expect(userItem).not.toBeVisible({ timeout: 5000 })
}

/**
 * Assert empty state is shown
 */
export async function assertEmptyState(page: Page, message: string) {
  const emptyElement = page.locator('.ant-empty-description', { hasText: new RegExp(message, 'i') })
  await expect(emptyElement).toBeVisible()
}

/**
 * Assert error message is shown
 */
export async function assertErrorMessage(page: Page, message: string) {
  const errorElement = page.locator('.ant-message-error', { hasText: new RegExp(message, 'i') })
  await expect(errorElement).toBeVisible({ timeout: 5000 })
}

/**
 * Assert success message is shown
 */
export async function assertSuccessMessage(page: Page, message: string) {
  const successElement = page.locator('.ant-message-success', { hasText: new RegExp(message, 'i') })
  await expect(successElement).toBeVisible({ timeout: 5000 })
}

/**
 * Assert pagination shows correct total
 */
export async function assertPaginationTotal(page: Page, expectedTotal: number) {
  const paginationText = page.locator('.ant-pagination-total-text')
  await expect(paginationText).toContainText(`${expectedTotal}`)
}

/**
 * Assert drawer is open with specific title
 */
export async function assertDrawerOpen(page: Page, titlePattern: string | RegExp) {
  const drawer = page.locator('.ant-drawer:visible')
  await expect(drawer).toBeVisible()

  const title = drawer.locator('.ant-drawer-title')
  if (typeof titlePattern === 'string') {
    await expect(title).toContainText(titlePattern)
  } else {
    await expect(title).toHaveText(titlePattern)
  }
}

/**
 * Assert drawer is closed
 */
export async function assertDrawerClosed(page: Page) {
  // Wait for drawer animation to complete
  await page.waitForTimeout(500)
  // Check that no visible drawer exists
  const drawer = page.locator('.ant-drawer-content')
  await expect(drawer).not.toBeVisible({ timeout: 5000 })
}
