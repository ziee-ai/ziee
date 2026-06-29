import { Locator, Page, expect } from '@playwright/test'
import { byTestId } from '../../testid'

/** Sonner success toast (i18n-safe via `data-type`). */
const expectSuccessToast = (page: Page) =>
  expect(
    page.locator('[data-sonner-toast][data-type="success"]').first(),
  ).toBeVisible({ timeout: 5000 })

/**
 * Flip the PermissionsField into "Advanced JSON" mode so the raw-array
 * textarea is mounted. Idempotent. `scope` may be the page or a drawer
 * locator — the switch testid is globally unique either way.
 */
export async function enableAdvancedPermissions(scope: Page | Locator) {
  const advancedSwitch = byTestId(scope, 'user-permissions-advanced-switch')
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
 * Create a new group through the UI.
 */
export async function createGroup(page: Page, groupData: CreateGroupData) {
  await byTestId(page, 'user-create-group-name-input').fill(groupData.name)

  if (groupData.description) {
    await byTestId(page, 'user-create-group-description-textarea').fill(
      groupData.description,
    )
  }

  if (groupData.permissions && groupData.permissions.length > 0) {
    await enableAdvancedPermissions(page)
    await byTestId(page, 'user-permissions-json-textarea').fill(
      JSON.stringify(groupData.permissions),
    )
  }

  await byTestId(page, 'user-create-group-submit-button').click()
  await expectSuccessToast(page)
}

/**
 * Update an existing group through the UI.
 */
export async function updateGroup(page: Page, groupData: UpdateGroupData) {
  if (groupData.name) {
    const nameField = byTestId(page, 'user-edit-group-name-input')
    await nameField.clear()
    await nameField.fill(groupData.name)
  }

  if (groupData.description !== undefined) {
    const descField = byTestId(page, 'user-edit-group-description-textarea')
    await descField.clear()
    await descField.fill(groupData.description)
  }

  if (groupData.permissions) {
    await enableAdvancedPermissions(page)
    const permissionsField = byTestId(page, 'user-permissions-json-textarea')
    await permissionsField.clear()
    await permissionsField.fill(JSON.stringify(groupData.permissions))
  }

  if (groupData.isActive !== undefined) {
    const activeSwitch = byTestId(page, 'user-edit-group-active-switch')
    const isCurrentlyActive =
      (await activeSwitch.getAttribute('aria-checked')) === 'true'
    if (isCurrentlyActive !== groupData.isActive) {
      await activeSwitch.click()
    }
  }

  await byTestId(page, 'user-edit-group-save-button').click()
  await expectSuccessToast(page)
}

/**
 * Delete a group through the UI (by name).
 */
export async function deleteGroup(page: Page, groupName: string) {
  const deleteButton = byTestId(page, `user-group-row-${groupName}`).locator(
    '[data-testid^="user-group-delete-button-"]',
  )
  await deleteButton.click()

  await page
    .locator(
      '[data-testid^="user-group-delete-confirm-"][data-testid$="-confirm"]',
    )
    .click()

  await expectSuccessToast(page)
}

/**
 * View group members (open the members drawer by group name).
 */
export async function viewGroupMembers(page: Page, groupName: string) {
  const membersButton = byTestId(page, `user-group-row-${groupName}`).locator(
    '[data-testid^="user-group-members-button-"]',
  )
  await membersButton.click()
  await byTestId(page, 'user-group-members-list').waitFor({ state: 'visible' })
}

/**
 * Assign the user to a group via the per-row Assign control in the open
 * UserGroupsDrawer (by group name).
 */
export async function assignUserToGroupInDrawer(page: Page, groupName: string) {
  const row = byTestId(page, `user-groups-drawer-row-${groupName}`)
  await row
    .locator('[data-testid^="user-groups-drawer-assign-row-button-"]')
    .click()
  await expect(
    row.locator('[data-testid^="user-groups-drawer-member-tag-"]'),
  ).toBeVisible({ timeout: 5000 })
}

/**
 * Remove the user from a group via the per-row Remove control + Confirm in the
 * open UserGroupsDrawer (by group name).
 */
export async function removeUserFromGroup(page: Page, groupName: string) {
  const row = byTestId(page, `user-groups-drawer-row-${groupName}`)
  await row
    .locator('[data-testid^="user-groups-drawer-remove-button-"]')
    .click()

  await page
    .locator(
      '[data-testid^="user-groups-drawer-remove-confirm-"][data-testid$="-confirm"]',
    )
    .click()

  await expectSuccessToast(page)
  await expect(
    row.locator('[data-testid^="user-groups-drawer-member-tag-"]'),
  ).toHaveCount(0, { timeout: 5000 })
}
