import { Page, expect } from '@playwright/test'
import { byTestId } from '../../testid'
import { enableAdvancedPermissions } from './group-actions'

export interface CreateUserData {
  username: string
  email: string
  password: string
  displayName?: string
  permissions?: string[]
}

export interface UpdateUserData {
  username?: string
  email?: string
  displayName?: string
  permissions?: string[]
}

/** Sonner success/error toasts carry `[data-type]`; assert on that (i18n-safe). */
export const expectSuccessToast = (page: Page) =>
  expect(
    page.locator('[data-sonner-toast][data-type="success"]').first(),
  ).toBeVisible({ timeout: 5000 })

export const expectErrorToast = (page: Page) =>
  expect(
    page.locator('[data-sonner-toast][data-type="error"]').first(),
  ).toBeVisible({ timeout: 5000 })

/**
 * Create a new user through the UI
 */
export async function createUser(page: Page, userData: CreateUserData) {
  await byTestId(page, 'user-create-username-input').fill(userData.username)
  await byTestId(page, 'user-create-email-input').fill(userData.email)
  await byTestId(page, 'user-create-password-input').fill(userData.password)

  if (userData.displayName) {
    await byTestId(page, 'user-create-display-name-input').fill(
      userData.displayName,
    )
  }

  if (userData.permissions && userData.permissions.length > 0) {
    await enableAdvancedPermissions(page)
    await byTestId(page, 'user-permissions-json-textarea').fill(
      JSON.stringify(userData.permissions),
    )
  }

  await byTestId(page, 'user-create-submit-button').click()
  await expectSuccessToast(page)
}

/**
 * Update an existing user through the UI
 *
 * Note: email + permissions are no longer editable in this drawer; callers
 * passing those fields are silently ignored.
 */
export async function updateUser(page: Page, userData: UpdateUserData) {
  if (userData.username) {
    await byTestId(page, 'user-edit-username-input').fill(userData.username)
  }

  if (userData.displayName !== undefined) {
    await byTestId(page, 'user-edit-display-name-input').fill(
      userData.displayName,
    )
  }

  await byTestId(page, 'user-edit-submit-button').click()
  await expectSuccessToast(page)
}

/**
 * Delete a user through the UI (by username).
 */
export async function deleteUser(page: Page, username: string) {
  const deleteButton = byTestId(page, `user-row-${username}`).locator(
    '[data-testid^="user-delete-button-"]',
  )
  await deleteButton.waitFor({ state: 'visible', timeout: 5000 })
  await deleteButton.click()

  // Confirm in the kit Confirm dialog (confirm button = `<testid>-confirm`).
  await page
    .locator(
      '[data-testid^="user-delete-confirm-"][data-testid$="-confirm"]',
    )
    .click()

  await expectSuccessToast(page)
}

/**
 * Toggle user active status through the UI (by username).
 */
export async function toggleUserStatus(page: Page, username: string) {
  const statusSwitch = byTestId(page, `user-row-${username}`).locator(
    '[data-testid^="user-active-switch-"]',
  )
  await statusSwitch.click()

  await page
    .locator(
      '[data-testid^="user-toggle-active-confirm-"][data-testid$="-confirm"]',
    )
    .click()

  await expectSuccessToast(page)
}

/**
 * Reset user password through the UI (by username).
 */
export async function resetUserPassword(
  page: Page,
  username: string,
  newPassword: string,
) {
  const resetButton = byTestId(page, `user-row-${username}`).locator(
    '[data-testid^="user-reset-password-button-"]',
  )
  await resetButton.waitFor({ state: 'visible', timeout: 5000 })
  await resetButton.click()

  await byTestId(page, 'user-reset-password-form').waitFor({ state: 'visible' })
  await byTestId(page, 'user-reset-new-password-input').fill(newPassword)
  await byTestId(page, 'user-reset-confirm-password-input').fill(newPassword)

  await byTestId(page, 'user-reset-password-submit-button').click()
  await expectSuccessToast(page)
}

/**
 * Assign a user to one or more groups via the per-group Assign buttons in the
 * user-groups drawer.
 */
export async function assignUserToGroups(
  page: Page,
  username: string,
  groupNames: string[],
) {
  const groupsButton = byTestId(page, `user-row-${username}`).locator(
    '[data-testid^="user-groups-button-"]',
  )
  await groupsButton.waitFor({ state: 'visible', timeout: 5000 })
  await groupsButton.click()

  await byTestId(page, 'user-groups-drawer-list').waitFor({ state: 'visible' })

  for (const groupName of groupNames) {
    const row = byTestId(page, `user-groups-drawer-row-${groupName}`)
    await row
      .locator('[data-testid^="user-groups-drawer-assign-row-button-"]')
      .click()
    await expect(
      row.locator('[data-testid^="user-groups-drawer-member-tag-"]'),
    ).toBeVisible({ timeout: 5000 })
  }
}
