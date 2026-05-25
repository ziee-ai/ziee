import { Page, expect } from '@playwright/test'

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

/**
 * Create a new user through the UI
 *
 * Scoped to the active drawer (`.ant-drawer-open`) because AntD leaves
 * closed drawers in the DOM; page-wide getByLabel matches inputs from
 * prior drawers and trips Playwright strict-mode.
 */
export async function createUser(page: Page, userData: CreateUserData) {
  const drawer = page.locator('.ant-drawer.ant-drawer-open')

  // Fill in the form
  await drawer.getByLabel(/username/i).fill(userData.username)
  await drawer.getByLabel(/email/i).fill(userData.email)
  await drawer.getByLabel(/^password/i).fill(userData.password)

  if (userData.displayName) {
    await drawer.getByLabel(/display name/i).fill(userData.displayName)
  }

  if (userData.permissions && userData.permissions.length > 0) {
    const permissionsField = drawer.getByLabel(/permissions.*json/i)
    await permissionsField.fill(JSON.stringify(userData.permissions))
  }

  // Submit the form. Target the primary button by class — submit
  // labels were standardised to verb-only ("Create User" → "Create",
  // per audit I-1/I-2), and verb-only labels are too generic to match
  // by role/name safely.
  const submitButton = drawer.locator('.ant-btn-primary[type="submit"]')
  await submitButton.click()

  // Wait for success message
  await expect(page.locator('.ant-message-success').first()).toBeVisible({ timeout: 5000 })

  // Wait for drawer to close
  await page.waitForTimeout(500)
}

/**
 * Update an existing user through the UI
 *
 * Note: email + permissions are no longer editable in this drawer
 * (03-user F-01 / F-03 closure). Callers passing those fields are
 * silently ignored — `UpdateUserData` keeps them on the type so existing
 * tests still typecheck, but the helper no-ops those branches.
 */
export async function updateUser(page: Page, userData: UpdateUserData) {
  const drawer = page.locator('.ant-drawer.ant-drawer-open')

  if (userData.username) {
    await drawer.getByLabel(/username/i).fill(userData.username)
  }

  // email + permissions intentionally not edited here.

  if (userData.displayName !== undefined) {
    await drawer.getByLabel(/display name/i).fill(userData.displayName)
  }

  // Submit the form. Label is now "Save" (verb-only per audit I-2);
  // target by primary-button class instead.
  const submitButton = drawer.locator('.ant-btn-primary[type="submit"]')
  await submitButton.click()

  // Wait for success message
  await expect(page.locator('.ant-message-success').first()).toBeVisible({ timeout: 5000 })

  // Wait for drawer to close
  await page.waitForTimeout(500)
}

/**
 * Delete a user through the UI
 *
 * The Delete button itself uniquely identifies the row — its
 * aria-label includes the username (e.g. "Delete admin").
 */
export async function deleteUser(page: Page, username: string) {
  const deleteButton = page.getByRole('button', {
    name: `Delete ${username}`,
    exact: true,
  })
  await deleteButton.waitFor({ state: 'visible', timeout: 5000 })
  await deleteButton.click()

  // Confirm the deletion in the popconfirm. Target the primary button
  // by class so the locator survives okText standardisation
  // ("Yes" → "Delete" / "Deactivate" per audit I-4).
  const confirmButton = page.locator('.ant-popconfirm:visible .ant-btn-primary')
  await confirmButton.click()

  // Wait for success message
  await expect(page.locator('.ant-message-success').first()).toBeVisible({ timeout: 5000 })

  // Wait for UI to update
  await page.waitForTimeout(500)
}

/**
 * Toggle user active status through the UI
 *
 * Anchor on the user-specific Delete button (its aria-label includes
 * the username) and walk to the switch via XPath sibling axis. The
 * switch precedes Edit/Reset/Groups/Delete in document order.
 */
export async function toggleUserStatus(page: Page, username: string) {
  const statusSwitch = page
    .getByRole('button', { name: `Delete ${username}`, exact: true })
    .locator('xpath=preceding-sibling::button[contains(@class, "ant-switch")]')
    .first()

  await statusSwitch.click()

  // Confirm the action in the popconfirm. Primary-button class is
  // stable across okText variations.
  const confirmButton = page.locator('.ant-popconfirm:visible .ant-btn-primary')
  await confirmButton.click()

  // Wait for success message
  await expect(page.locator('.ant-message-success').first()).toBeVisible({ timeout: 5000 })

  // Wait for UI to update
  await page.waitForTimeout(500)
}

/**
 * Reset user password through the UI
 */
export async function resetUserPassword(
  page: Page,
  username: string,
  newPassword: string
) {
  // Open reset password drawer — same Delete-button anchor approach
  // as toggleUserStatus.
  const resetButton = page.getByRole('button', { name: new RegExp(`reset password.*${username}`, 'i') })
    .or(
      page
        .getByRole('button', { name: `Delete ${username}`, exact: true })
        .locator('xpath=preceding-sibling::button[normalize-space()="Reset Password"]')
    )

  await resetButton.first().click()

  // Wait for drawer
  const drawer = page.locator('.ant-drawer.ant-drawer-open', { hasText: /reset password/i })
  await drawer.waitFor({ state: 'visible' })

  // Fill in new password — scoped to drawer to avoid strict-mode flake
  // (closed drawers from earlier steps remain in the DOM).
  await drawer.getByLabel(/new password/i).fill(newPassword)

  // Submit the form. Label is now "Reset" (verb-only per audit I-2);
  // target by primary-button class instead.
  const submitButton = drawer.locator('.ant-btn-primary[type="submit"]')
  await submitButton.click()

  // Wait for success message
  await expect(page.locator('.ant-message-success').first()).toBeVisible({ timeout: 5000 })

  // Wait for drawer to close
  await page.waitForTimeout(500)
}

/**
 * Assign user to groups
 */
export async function assignUserToGroups(
  page: Page,
  username: string,
  groupNames: string[]
) {
  // Open user groups drawer — same Delete-button anchor approach.
  const groupsButton = page.getByRole('button', { name: new RegExp(`groups.*${username}`, 'i') })
    .or(
      page
        .getByRole('button', { name: `Delete ${username}`, exact: true })
        .locator('xpath=preceding-sibling::button[normalize-space()="Groups"]')
    )

  await groupsButton.first().click()

  // Wait for drawer
  const drawer = page.locator('.ant-drawer.ant-drawer-open')
  await drawer.waitFor({ state: 'visible' })

  // Click assign groups button
  const assignButton = drawer.getByRole('button', { name: /assign.*group/i })
  await assignButton.click()

  // Wait for assign drawer to open
  await page.waitForTimeout(300)

  // Select groups (implementation depends on the actual UI component)
  for (const groupName of groupNames) {
    const groupCheckbox = page.locator('.ant-drawer.ant-drawer-open').getByText(groupName)
    await groupCheckbox.click()
  }

  // Submit. Label is now "Assign" (verb-only per audit I-2); scope by
  // primary-button class to avoid colliding with the "Assign group"
  // CTA aria-label on parent UserGroupsDrawer.
  const submitButton = page.locator('.ant-drawer.ant-drawer-open .ant-btn-primary[type="submit"]')
  await submitButton.click()

  // Wait for success message
  await expect(page.locator('.ant-message-success').first()).toBeVisible({ timeout: 5000 })

  // Wait for UI to update
  await page.waitForTimeout(500)
}
