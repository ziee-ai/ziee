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
 */
export async function createUser(page: Page, userData: CreateUserData) {
  // Fill in the form
  await page.getByLabel(/username/i).fill(userData.username)
  await page.getByLabel(/email/i).fill(userData.email)
  await page.getByLabel(/^password/i).fill(userData.password)

  if (userData.displayName) {
    await page.getByLabel(/display name/i).fill(userData.displayName)
  }

  if (userData.permissions && userData.permissions.length > 0) {
    const permissionsField = page.getByLabel(/permissions.*json/i)
    await permissionsField.fill(JSON.stringify(userData.permissions))
  }

  // Submit the form
  const submitButton = page.locator('.ant-drawer:visible').getByRole('button', { name: /create user/i })
  await submitButton.click()

  // Wait for success message
  await expect(page.locator('.ant-message-success')).toBeVisible({ timeout: 5000 })

  // Wait for drawer to close
  await page.waitForTimeout(500)
}

/**
 * Update an existing user through the UI
 */
export async function updateUser(page: Page, userData: UpdateUserData) {
  if (userData.username) {
    await page.getByLabel(/username/i).fill(userData.username)
  }

  if (userData.email) {
    await page.getByLabel(/email/i).fill(userData.email)
  }

  if (userData.displayName !== undefined) {
    await page.getByLabel(/display name/i).fill(userData.displayName)
  }

  if (userData.permissions) {
    const permissionsField = page.getByLabel(/permissions.*json/i)
    await permissionsField.clear()
    await permissionsField.fill(JSON.stringify(userData.permissions))
  }

  // Submit the form
  const submitButton = page.locator('.ant-drawer:visible').getByRole('button', { name: /update user/i })
  await submitButton.click()

  // Wait for success message
  await expect(page.locator('.ant-message-success')).toBeVisible({ timeout: 5000 })

  // Wait for drawer to close
  await page.waitForTimeout(500)
}

/**
 * Delete a user through the UI
 */
export async function deleteUser(page: Page, username: string) {
  // Find username element, go up to user section, then find Delete button
  const usernameEl = page.locator('.ant-typography.font-medium', { hasText: username }).first()

  // Go up to the user info section container (2 levels up from username text)
  const userSection = usernameEl.locator('../..')

  // Find Delete button within that section
  const deleteButton = userSection.getByRole('button', { name: /^delete$/i })
  await deleteButton.waitFor({ state: 'visible', timeout: 5000 })
  await deleteButton.click()

  // Confirm the deletion in the popconfirm
  const confirmButton = page.locator('.ant-popconfirm:visible').getByRole('button', { name: /yes/i })
  await confirmButton.click()

  // Wait for success message
  await expect(page.locator('.ant-message-success').first()).toBeVisible({ timeout: 5000 })

  // Wait for UI to update
  await page.waitForTimeout(500)
}

/**
 * Toggle user active status through the UI
 */
export async function toggleUserStatus(page: Page, username: string) {
  // Find the switch for the specific user
  const userRow = page.locator(`text="${username}"`).locator('..')
  const statusSwitch = userRow.locator('button.ant-switch').or(userRow.locator('.ant-switch'))

  await statusSwitch.first().click()

  // Confirm the action in the popconfirm
  const confirmButton = page.locator('.ant-popconfirm:visible').getByRole('button', { name: /yes/i })
  await confirmButton.click()

  // Wait for success message
  await expect(page.locator('.ant-message-success')).toBeVisible({ timeout: 5000 })

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
  // Open reset password drawer
  const resetButton = page.getByRole('button', { name: new RegExp(`reset password.*${username}`, 'i') })
    .or(page.locator(`text="${username}"`).locator('..').getByRole('button', { name: /reset password/i }))

  await resetButton.first().click()

  // Wait for drawer
  const drawer = page.locator('.ant-drawer:visible', { hasText: /reset password/i })
  await drawer.waitFor({ state: 'visible' })

  // Fill in new password
  await page.getByLabel(/new password/i).fill(newPassword)

  // Submit the form
  const submitButton = drawer.getByRole('button', { name: /reset password/i })
  await submitButton.click()

  // Wait for success message
  await expect(page.locator('.ant-message-success')).toBeVisible({ timeout: 5000 })

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
  // Open user groups drawer
  const groupsButton = page.getByRole('button', { name: new RegExp(`groups.*${username}`, 'i') })
    .or(page.locator(`text="${username}"`).locator('..').getByRole('button', { name: /groups/i }))

  await groupsButton.first().click()

  // Wait for drawer
  const drawer = page.locator('.ant-drawer:visible')
  await drawer.waitFor({ state: 'visible' })

  // Click assign groups button
  const assignButton = drawer.getByRole('button', { name: /assign.*group/i })
  await assignButton.click()

  // Wait for assign drawer to open
  await page.waitForTimeout(300)

  // Select groups (implementation depends on the actual UI component)
  for (const groupName of groupNames) {
    const groupCheckbox = page.locator('.ant-drawer:visible').getByText(groupName)
    await groupCheckbox.click()
  }

  // Submit
  const submitButton = page.locator('.ant-drawer:visible').getByRole('button', { name: /assign/i }).last()
  await submitButton.click()

  // Wait for success message
  await expect(page.locator('.ant-message-success')).toBeVisible({ timeout: 5000 })

  // Wait for UI to update
  await page.waitForTimeout(500)
}
