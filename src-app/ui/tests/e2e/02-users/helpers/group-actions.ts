import { Locator, Page, expect } from '@playwright/test'

/**
 * The permissions field defaults to the searchable picker. Flip it into
 * "Advanced JSON" mode so the raw-array textarea (aria-label
 * "Permissions (JSON Array)") is mounted and fillable. Idempotent — reads
 * the switch's aria-checked before toggling.
 */
export async function enableAdvancedPermissions(drawer: Locator) {
  const advancedSwitch = drawer.getByRole('switch', { name: /advanced json/i })
  if ((await advancedSwitch.getAttribute('aria-checked')) !== 'true') {
    await advancedSwitch.click()
  }
}

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
 *
 * All getByLabel calls are scoped to the active drawer because AntD
 * leaves closed drawers in the DOM; an unscoped page.getByLabel matches
 * inputs from BOTH the previously-closed Create drawer and the now-open
 * Edit drawer and trips strict-mode.
 */
export async function createGroup(page: Page, groupData: CreateGroupData) {
  const drawer = page.locator('.ant-drawer.ant-drawer-open')

  // Fill in the form
  await drawer.getByLabel(/group name/i).fill(groupData.name)

  if (groupData.description) {
    await drawer.getByLabel(/description/i).fill(groupData.description)
  }

  if (groupData.permissions && groupData.permissions.length > 0) {
    await enableAdvancedPermissions(drawer)
    const permissionsField = drawer.getByLabel(/permissions.*json/i)
    await permissionsField.fill(JSON.stringify(groupData.permissions))
  }

  // Submit the form. Label is now "Create" (verb-only per audit I-2);
  // target by primary-button class instead.
  const submitButton = drawer.locator('.ant-btn-primary[type="submit"]')
  await submitButton.click()

  // Wait for success message
  await expect(page.locator('.ant-message-success').first()).toBeVisible({ timeout: 5000 })

  // Wait for drawer to close
  await page.waitForTimeout(500)
}

/**
 * Update an existing group through the UI
 *
 * Scoped to the active drawer for the same reason as createGroup above.
 */
export async function updateGroup(page: Page, groupData: UpdateGroupData) {
  const drawer = page.locator('.ant-drawer.ant-drawer-open')

  if (groupData.name) {
    const nameField = drawer.getByLabel(/group name/i)
    await nameField.clear()
    await nameField.fill(groupData.name)
  }

  if (groupData.description !== undefined) {
    const descField = drawer.getByLabel(/description/i)
    await descField.clear()
    await descField.fill(groupData.description)
  }

  if (groupData.permissions) {
    await enableAdvancedPermissions(drawer)
    const permissionsField = drawer.getByLabel(/permissions.*json/i)
    await permissionsField.clear()
    await permissionsField.fill(JSON.stringify(groupData.permissions))
  }

  if (groupData.isActive !== undefined) {
    // The Switch carries aria-label="Set group as active or inactive"
    // which is the canonical accessible name. AntD renders the Switch
    // as <button role="switch" aria-checked="true|false"> — read the
    // attribute directly rather than `.isChecked()` which assumes a
    // checkbox role.
    const activeSwitch = drawer.getByLabel('Set group as active or inactive')
    const ariaChecked = await activeSwitch.getAttribute('aria-checked')
    const isCurrentlyActive = ariaChecked === 'true'

    if (isCurrentlyActive !== groupData.isActive) {
      await activeSwitch.click()
    }
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
 * Delete a group through the UI
 */
export async function deleteGroup(page: Page, groupName: string) {
  // Find the delete button for the specific group (button lives 2
  // levels up from name text — same layout as user rows).
  const deleteButton = page.getByRole('button', { name: new RegExp(`delete.*${groupName}`, 'i') })
    .or(page.locator(`text="${groupName}"`).first().locator('../..').getByRole('button', { name: /delete/i }))

  await deleteButton.first().click()

  // Confirm the deletion in the popconfirm. Target the primary button
  // by class so the locator survives okText standardisation
  // ("Yes" → "Delete" / "Remove" per audit I-4).
  const confirmButton = page.locator('.ant-popconfirm:visible .ant-btn-primary')
  await confirmButton.click()

  // Wait for success message
  await expect(page.locator('.ant-message-success').first()).toBeVisible({ timeout: 5000 })

  // Wait for UI to update
  await page.waitForTimeout(500)
}

/**
 * View group members
 */
export async function viewGroupMembers(page: Page, groupName: string) {
  const membersButton = page.getByRole('button', { name: new RegExp(`members.*${groupName}`, 'i') })
    .or(page.locator(`text="${groupName}"`).first().locator('../..').getByRole('button', { name: /members/i }))

  await membersButton.first().click()

  // Wait for drawer to appear
  const drawer = page.locator('.ant-drawer.ant-drawer-open', { hasText: /members of/i })
  await drawer.waitFor({ state: 'visible' })
}

/**
 * Remove user from group (from group members drawer)
 */
export async function removeUserFromGroup(page: Page, username: string) {
  // Find remove button for the user in the drawer
  const userItem = page.locator('.ant-drawer.ant-drawer-open').locator('.ant-list-item', { hasText: username })
  const removeButton = userItem.getByRole('button', { name: /remove/i })

  await removeButton.click()

  // Confirm if popconfirm appears. Primary-button class is stable
  // across okText variations.
  const confirmButton = page.locator('.ant-popconfirm:visible .ant-btn-primary')
  if (await confirmButton.isVisible()) {
    await confirmButton.click()
  }

  // Wait for success message
  await expect(page.locator('.ant-message-success').first()).toBeVisible({ timeout: 5000 })

  // Wait for UI to update
  await page.waitForTimeout(500)
}
