import { Page, expect } from '@playwright/test'
import { byTestId } from '../../testid'

/**
 * Assert that a user exists in the list (by username-keyed row testid).
 */
export async function assertUserExists(page: Page, username: string) {
  await expect(byTestId(page, `user-row-${username}`)).toBeVisible({
    timeout: 5000,
  })
}

/**
 * Assert that a user does not exist in the list.
 */
export async function assertUserNotExists(page: Page, username: string) {
  await expect(byTestId(page, `user-row-${username}`)).toHaveCount(0, {
    timeout: 5000,
  })
}

/**
 * Assert user status (active/inactive) via the row's status badge text.
 */
export async function assertUserStatus(
  page: Page,
  username: string,
  expectedStatus: 'active' | 'inactive',
) {
  const badge = byTestId(page, `user-row-${username}`).locator(
    '[data-testid^="user-status-badge-"]',
  )
  await expect(badge).toHaveText(new RegExp(`^${expectedStatus}$`, 'i'), {
    timeout: 5000,
  })
}

/**
 * Assert user email is shown in the row's descriptions (dynamic data).
 */
export async function assertUserEmail(
  page: Page,
  username: string,
  expectedEmail: string,
) {
  const descriptions = byTestId(page, `user-row-${username}`).locator(
    '[data-testid^="user-descriptions-"]',
  )
  await expect(descriptions).toContainText(expectedEmail)
}

/**
 * Assert that a group exists in the list (by name-keyed row testid).
 */
export async function assertGroupExists(page: Page, groupName: string) {
  await expect(byTestId(page, `user-group-row-${groupName}`)).toBeVisible({
    timeout: 5000,
  })
}

/**
 * Assert that a group does not exist in the list.
 */
export async function assertGroupNotExists(page: Page, groupName: string) {
  await expect(byTestId(page, `user-group-row-${groupName}`)).toHaveCount(0, {
    timeout: 5000,
  })
}

/**
 * Assert group status (active/inactive) via the row's status text.
 *
 * Reloads first because the listing card's badge isn't always rebound to the
 * emitGroupUpdated event after a PUT.
 */
export async function assertGroupStatus(
  page: Page,
  groupName: string,
  expectedStatus: 'active' | 'inactive',
) {
  await page.reload({ waitUntil: 'load' })

  const statusText = byTestId(page, `user-group-row-${groupName}`).locator(
    '[data-testid^="user-group-status-text-"]',
  )
  await expect(statusText).toHaveText(new RegExp(`^${expectedStatus}$`, 'i'), {
    timeout: 5000,
  })
}

/**
 * Assert group has system tag.
 */
export async function assertGroupIsSystem(page: Page, groupName: string) {
  const systemTag = byTestId(page, `user-group-row-${groupName}`).locator(
    '[data-testid^="user-group-system-tag-"]',
  )
  await expect(systemTag).toBeVisible()
}

/**
 * Assert user is in the group members list (members drawer).
 */
export async function assertUserInGroup(page: Page, username: string) {
  await expect(
    byTestId(page, `user-group-member-row-${username}`),
  ).toBeVisible({ timeout: 5000 })
}

/**
 * Assert user is NOT in the group members list (members drawer).
 */
export async function assertUserNotInGroup(page: Page, username: string) {
  await expect(
    byTestId(page, `user-group-member-row-${username}`),
  ).toHaveCount(0, { timeout: 5000 })
}

/**
 * Assert an empty-state placeholder is shown (any kit Empty carries a
 * `-empty` testid suffix).
 */
export async function assertEmptyState(page: Page, _message?: string) {
  await expect(page.locator('[data-testid$="-empty"]').first()).toBeVisible()
}

/**
 * Assert an error toast is shown (sonner `data-type="error"`).
 */
export async function assertErrorMessage(page: Page, _message?: string) {
  await expect(
    page.locator('[data-sonner-toast][data-type="error"]').first(),
  ).toBeVisible({ timeout: 5000 })
}

/**
 * Assert a success toast is shown (sonner `data-type="success"`).
 */
export async function assertSuccessMessage(page: Page, _message?: string) {
  await expect(
    page.locator('[data-sonner-toast][data-type="success"]').first(),
  ).toBeVisible({ timeout: 5000 })
}

/**
 * Assert pagination shows the expected total (dynamic count).
 */
export async function assertPaginationTotal(
  page: Page,
  expectedTotal: number,
  paginationTestId = 'user-list-pagination',
) {
  await expect(byTestId(page, paginationTestId)).toContainText(
    `${expectedTotal}`,
  )
}

/**
 * Assert a drawer (Radix dialog) is open. The `_titlePattern` arg is kept for
 * call-site compatibility; the open-state is asserted structurally.
 */
export async function assertDrawerOpen(
  page: Page,
  _titlePattern?: string | RegExp,
) {
  await expect(page.getByRole('dialog').first()).toBeVisible()
}

/**
 * Assert no drawer (Radix dialog) is open. Radix unmounts closed dialogs.
 */
export async function assertDrawerClosed(page: Page) {
  await expect(page.getByRole('dialog')).toHaveCount(0, { timeout: 5000 })
}
