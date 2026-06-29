import { Page } from '@playwright/test'
import { byTestId } from '../../testid'

/**
 * Page-object-style navigation helpers for the Users / User Groups admin
 * surfaces. All selectors are testid-based (i18n-safe). Per-row action
 * buttons carry id-suffixed testids (`user-edit-button-<id>` etc.); since
 * callers only know the username/group name, we scope to the name-keyed row
 * wrapper (`user-row-<username>` / `user-group-row-<name>` /
 * `user-groups-drawer-row-<name>`, added at source) and then target the
 * action by its testid prefix — unique within the scoped row.
 */

const usersHeading = (page: Page) => byTestId(page, 'user-list-card')

/**
 * Navigate to the users settings page
 */
export async function navigateToUsers(page: Page, baseURL: string) {
  await page.goto(`${baseURL}/settings/users`)
  await page.waitForLoadState('load')
  await usersHeading(page).waitFor({ timeout: 10000 })
}

/**
 * Navigate to the user groups settings page
 */
export async function navigateToUserGroups(page: Page, baseURL: string) {
  await page.goto(`${baseURL}/settings/user-groups`)
  await page.waitForLoadState('load')
  // Either the create button (always present for admins) or the empty state.
  await byTestId(page, 'user-groups-create-button')
    .or(byTestId(page, 'user-groups-empty'))
    .first()
    .waitFor({ timeout: 10000 })
}

/**
 * Open the create user drawer
 */
export async function openCreateUserDrawer(page: Page) {
  await byTestId(page, 'user-create-open-button').click()
  await byTestId(page, 'user-create-form').waitFor({ state: 'visible' })
}

/**
 * Open the create group drawer
 */
export async function openCreateGroupDrawer(page: Page) {
  await byTestId(page, 'user-groups-create-button').click()
  await byTestId(page, 'user-create-group-form').waitFor({ state: 'visible' })
}

/**
 * Open the edit user drawer for a specific user (by username).
 */
export async function openEditUserDrawer(page: Page, username: string) {
  const editButton = byTestId(page, `user-row-${username}`).locator(
    '[data-testid^="user-edit-button-"]',
  )
  await editButton.waitFor({ state: 'visible', timeout: 5000 })
  await editButton.click()
  await byTestId(page, 'user-edit-form').waitFor({ state: 'visible' })
}

/**
 * Open the edit group drawer for a specific group (by name).
 */
export async function openEditGroupDrawer(page: Page, groupName: string) {
  const editButton = byTestId(page, `user-group-row-${groupName}`).locator(
    '[data-testid^="user-group-edit-button-"]',
  )
  await editButton.waitFor({ state: 'visible', timeout: 5000 })
  await editButton.click()
  await byTestId(page, 'user-edit-group-form').waitFor({ state: 'visible' })
}

/**
 * Open the groups drawer for a specific user (by username).
 */
export async function openUserGroupsDrawer(page: Page, username: string) {
  const groupsButton = byTestId(page, `user-row-${username}`).locator(
    '[data-testid^="user-groups-button-"]',
  )
  await groupsButton.waitFor({ state: 'visible', timeout: 5000 })
  await groupsButton.click()
  await byTestId(page, 'user-groups-drawer-list')
    .or(byTestId(page, 'user-groups-drawer-empty'))
    .first()
    .waitFor({ state: 'visible' })
}

/**
 * Open the members drawer for a specific group (by name).
 */
export async function openGroupMembersDrawer(page: Page, groupName: string) {
  const membersButton = byTestId(page, `user-group-row-${groupName}`).locator(
    '[data-testid^="user-group-members-button-"]',
  )
  await membersButton.waitFor({ state: 'visible', timeout: 5000 })
  await membersButton.click()
  await byTestId(page, 'user-group-members-list').waitFor({ state: 'visible' })
}

/**
 * Close any open drawer via the kit Drawer's "Close drawer" button.
 */
export async function closeDrawer(page: Page) {
  const closeButton = byTestId(page, 'layout-drawer-close-button')
  if (await closeButton.isVisible()) {
    await closeButton.click()
    await closeButton.waitFor({ state: 'hidden', timeout: 5000 })
  }
}
