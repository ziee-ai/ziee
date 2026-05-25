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
 *
 * `.first()` on both ends: the username text node can be matched in
 * multiple places (nav menu, breadcrumb, card title), and the user
 * card itself can contain nested badges. The user's own row is always
 * first in document order, so .first() is the right disambiguation.
 */
export async function assertUserStatus(
  page: Page,
  username: string,
  expectedStatus: 'active' | 'inactive'
) {
  const userRow = page.locator(`text="${username}"`).first().locator('..')
  const statusBadge = userRow.locator('.ant-badge-status-text').first()

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
 *
 * `.first()` on the matched cards and the badge: `.ant-card` may match
 * both the outer group card and an inner card nested inside it, and
 * the badge selector then resolves to multiple. The first match in
 * document order is the group's own row.
 */
export async function assertGroupStatus(
  page: Page,
  groupName: string,
  expectedStatus: 'active' | 'inactive'
) {
  // Reload the page to ensure the listing widget re-fetches and
  // displays the latest group state — the UserGroups store emits
  // emitGroupUpdated after a successful PUT but the listing card's
  // badge isn't always rebound to that event.
  await page.reload({ waitUntil: 'networkidle' })

  // Locate the group's row by its unique Edit button (aria-label
  // includes the group name) and walk up to the row container, then
  // back down to the badge. Plain `.ant-card.filter(hasText:...)`
  // matches both the outer system-groups card and the row.
  const editButton = page.getByRole('button', {
    name: `Edit ${groupName}`,
    exact: true,
  })
  const groupRow = editButton.locator(
    'xpath=ancestor::div[contains(@class, "ant-card-body")][1]'
  )
  const statusBadge = groupRow.locator('.ant-badge-status-text').first()

  await expect(statusBadge).toHaveText(new RegExp(expectedStatus, 'i'), {
    timeout: 5000,
  })
}

/**
 * Assert group has system tag
 */
export async function assertGroupIsSystem(page: Page, groupName: string) {
  const groupCard = page.locator('.ant-card', { hasText: groupName }).first()
  const systemTag = groupCard.locator('.ant-tag', { hasText: /system/i }).first()
  await expect(systemTag).toBeVisible()
}

/**
 * Assert user is in group members list
 */
export async function assertUserInGroup(page: Page, username: string) {
  const drawer = page.locator('.ant-drawer.ant-drawer-open')
  const userItem = drawer.locator('.ant-list-item', { hasText: username })
  await expect(userItem).toBeVisible({ timeout: 5000 })
}

/**
 * Assert user is not in group members list
 */
export async function assertUserNotInGroup(page: Page, username: string) {
  const drawer = page.locator('.ant-drawer.ant-drawer-open')
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
 *
 * Use `.ant-drawer-open` rather than `.ant-drawer:visible`: AntD leaves
 * closed drawers in the DOM with the wrapper marked as content-hidden
 * but the outer node still considered "visible" by Playwright, so
 * `:visible` matches 2+ drawers and strict-mode fails. Only the
 * actively-open drawer carries `ant-drawer-open`.
 */
export async function assertDrawerOpen(page: Page, titlePattern: string | RegExp) {
  const drawer = page.locator('.ant-drawer.ant-drawer-open')
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
 *
 * AntD keeps the .ant-drawer-content node in DOM after close (just with
 * the wrapper hidden) — so check that no drawer carries the
 * ant-drawer-open class instead.
 */
export async function assertDrawerClosed(page: Page) {
  // Wait for drawer animation to complete
  await page.waitForTimeout(500)
  // Check that no active drawer is open
  const drawer = page.locator('.ant-drawer.ant-drawer-open')
  await expect(drawer).not.toBeVisible({ timeout: 5000 })
}
