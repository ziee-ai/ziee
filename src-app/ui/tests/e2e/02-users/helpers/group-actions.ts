import { Page, expect } from '@playwright/test'

export interface CreateGroupData {
  name: string
  description?: string
  permissions?: string[]
}

export interface UpdateGroupData {
  name?: string
  description?: string
  permissions?: string[]
  isActive?: boolean
}

/**
 * Create a new group through the UI
 */
export async function createGroup(page: Page, groupData: CreateGroupData) {
  // Fill in the form
  await page.getByLabel(/group name/i).fill(groupData.name)

  if (groupData.description) {
    await page.getByLabel(/description/i).fill(groupData.description)
  }

  if (groupData.permissions && groupData.permissions.length > 0) {
    const permissionsField = page.getByLabel(/permissions.*json/i)
    await permissionsField.fill(JSON.stringify(groupData.permissions))
  }

  // Submit the form
  const submitButton = page.locator('.ant-drawer:visible').getByRole('button', { name: /create group/i })
  await submitButton.click()

  // Wait for success message
  await expect(page.locator('.ant-message-success')).toBeVisible({ timeout: 5000 })

  // Wait for drawer to close
  await page.waitForTimeout(500)
}

/**
 * Update an existing group through the UI
 */
export async function updateGroup(page: Page, groupData: UpdateGroupData) {
  if (groupData.name) {
    const nameField = page.getByLabel(/group name/i)
    await nameField.clear()
    await nameField.fill(groupData.name)
  }

  if (groupData.description !== undefined) {
    const descField = page.getByLabel(/description/i)
    await descField.clear()
    await descField.fill(groupData.description)
  }

  if (groupData.permissions) {
    const permissionsField = page.getByLabel(/permissions.*json/i)
    await permissionsField.clear()
    await permissionsField.fill(JSON.stringify(groupData.permissions))
  }

  if (groupData.isActive !== undefined) {
    const activeSwitch = page.locator('.ant-drawer:visible').getByLabel(/active/i)
    const isCurrentlyActive = await activeSwitch.isChecked()

    if (isCurrentlyActive !== groupData.isActive) {
      await activeSwitch.click()
    }
  }

  // Submit the form
  const submitButton = page.locator('.ant-drawer:visible').getByRole('button', { name: /update group/i })
  await submitButton.click()

  // Wait for success message
  await expect(page.locator('.ant-message-success')).toBeVisible({ timeout: 5000 })

  // Wait for drawer to close
  await page.waitForTimeout(500)
}

/**
 * Delete a group through the UI
 */
export async function deleteGroup(page: Page, groupName: string) {
  // Find the delete button for the specific group
  const deleteButton = page.getByRole('button', { name: new RegExp(`delete.*${groupName}`, 'i') })
    .or(page.locator(`text="${groupName}"`).locator('..').getByRole('button', { name: /delete/i }))

  await deleteButton.first().click()

  // Confirm the deletion in the popconfirm
  const confirmButton = page.locator('.ant-popconfirm:visible').getByRole('button', { name: /yes/i })
  await confirmButton.click()

  // Wait for success message
  await expect(page.locator('.ant-message-success')).toBeVisible({ timeout: 5000 })

  // Wait for UI to update
  await page.waitForTimeout(500)
}

/**
 * View group members
 */
export async function viewGroupMembers(page: Page, groupName: string) {
  const membersButton = page.getByRole('button', { name: new RegExp(`members.*${groupName}`, 'i') })
    .or(page.locator(`text="${groupName}"`).locator('..').getByRole('button', { name: /members/i }))

  await membersButton.first().click()

  // Wait for drawer to appear
  const drawer = page.locator('.ant-drawer:visible', { hasText: /members of/i })
  await drawer.waitFor({ state: 'visible' })
}

/**
 * Remove user from group (from group members drawer)
 */
export async function removeUserFromGroup(page: Page, username: string) {
  // Find remove button for the user in the drawer
  const userItem = page.locator('.ant-drawer:visible').locator('.ant-list-item', { hasText: username })
  const removeButton = userItem.getByRole('button', { name: /remove/i })

  await removeButton.click()

  // Confirm if popconfirm appears
  const confirmButton = page.locator('.ant-popconfirm:visible').getByRole('button', { name: /yes/i })
  if (await confirmButton.isVisible()) {
    await confirmButton.click()
  }

  // Wait for success message
  await expect(page.locator('.ant-message-success')).toBeVisible({ timeout: 5000 })

  // Wait for UI to update
  await page.waitForTimeout(500)
}
