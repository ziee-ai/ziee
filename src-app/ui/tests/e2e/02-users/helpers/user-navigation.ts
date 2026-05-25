import { Page } from '@playwright/test'

/**
 * Navigate to the users settings page
 */
export async function navigateToUsers(page: Page, baseURL: string) {
  await page.goto(`${baseURL}/settings/users`)
  await page.waitForLoadState('load')
  // Wait for page heading to ensure page is loaded
  await page.getByRole('heading', { name: /^users$/i }).waitFor({ timeout: 10000 })
}

/**
 * Navigate to the user groups settings page
 */
export async function navigateToUserGroups(page: Page, baseURL: string) {
  await page.goto(`${baseURL}/settings/user-groups`)
  await page.waitForLoadState('load')
  // Wait for page heading to ensure page is loaded
  await page.getByRole('heading', { name: /user groups/i }).waitFor({ timeout: 10000 })
}

/**
 * Open the create user drawer
 *
 * The trigger button matches /create user/i but so does the submit
 * button inside a previously-closed drawer (AntD keeps closed drawers
 * in DOM). `.first()` grabs the trigger in document order.
 */
export async function openCreateUserDrawer(page: Page) {
  const createButton = page.getByRole('button', { name: /create user/i }).first()
  await createButton.click()

  // Wait for drawer to appear
  const drawer = page.locator('.ant-drawer.ant-drawer-open', { hasText: 'Create User' })
  await drawer.waitFor({ state: 'visible' })
}

/**
 * Open the create group drawer
 *
 * Same flake pattern as openCreateUserDrawer — `.first()` to disambiguate.
 */
export async function openCreateGroupDrawer(page: Page) {
  const createButton = page.getByRole('button', { name: /create group/i }).first()
  await createButton.click()

  // Wait for drawer to appear
  const drawer = page.locator('.ant-drawer.ant-drawer-open', { hasText: 'Create User Group' })
  await drawer.waitFor({ state: 'visible' })
}

/**
 * Open the edit user drawer for a specific user
 *
 * Find the row container directly: the row has classes
 * "flex items-center gap-2 mb-2 flex-wrap" and contains both the
 * username span and the action buttons. `:has()` filters reliably
 * without brittle ancestor counting.
 */
export async function openEditUserDrawer(page: Page, username: string) {
  // Find the user-specific Delete button (its aria-label includes the
  // username, so it's unique). The Edit button is its sibling in the
  // buttons div — use sibling axis instead of parent + descendant
  // because some Playwright versions are flaky about `..` chaining.
  const editButton = page
    .getByRole('button', { name: `Delete ${username}`, exact: true })
    .locator('xpath=preceding-sibling::button[normalize-space()="Edit"]')
    .first()
  await editButton.waitFor({ state: 'visible', timeout: 5000 })
  await editButton.click()

  // Wait for drawer to appear
  const drawer = page.locator('.ant-drawer.ant-drawer-open', { hasText: /edit user/i })
  await drawer.waitFor({ state: 'visible' })
}

/**
 * Open the edit group drawer for a specific group
 */
export async function openEditGroupDrawer(page: Page, groupName: string) {
  // Edit button lives 2 levels up from group name text (group info
  // section and button section share a common parent).
  const editButton = page.getByRole('button', { name: new RegExp(`edit.*${groupName}`, 'i') })
    .or(page.locator(`text="${groupName}"`).first().locator('../..').getByRole('button', { name: /edit/i }))

  await editButton.first().click()

  // Wait for drawer to appear
  const drawer = page.locator('.ant-drawer.ant-drawer-open', { hasText: /edit group/i })
  await drawer.waitFor({ state: 'visible' })
}

/**
 * Open the groups drawer for a specific user
 */
export async function openUserGroupsDrawer(page: Page, username: string) {
  // Anchor on user-specific Delete button (aria-label includes the
  // username) and use sibling axis to reach the Groups button.
  const groupsButton = page.getByRole('button', { name: new RegExp(`groups.*${username}`, 'i') })
    .or(
      page
        .getByRole('button', { name: `Delete ${username}`, exact: true })
        .locator('xpath=preceding-sibling::button[normalize-space()="Groups"]')
    )

  await groupsButton.first().click()

  // Wait for drawer to appear
  const drawer = page.locator('.ant-drawer.ant-drawer-open')
  await drawer.waitFor({ state: 'visible' })
}

/**
 * Open the members drawer for a specific group
 */
export async function openGroupMembersDrawer(page: Page, groupName: string) {
  const membersButton = page.getByRole('button', { name: new RegExp(`members.*${groupName}`, 'i') })
    .or(page.locator(`text="${groupName}"`).first().locator('../..').getByRole('button', { name: /members/i }))

  await membersButton.first().click()

  // Wait for drawer to appear
  const drawer = page.locator('.ant-drawer.ant-drawer-open', { hasText: /members of/i })
  await drawer.waitFor({ state: 'visible' })
}

/**
 * Close any open drawer
 *
 * The custom Drawer wrapper renders an aria-labelled "Close drawer"
 * button in the title slot instead of the default .ant-drawer-close.
 */
export async function closeDrawer(page: Page) {
  const closeButton = page
    .locator('.ant-drawer.ant-drawer-open')
    .getByRole('button', { name: 'Close drawer' })
  if (await closeButton.isVisible()) {
    await closeButton.click()
    // Wait for drawer animation to complete
    await page.waitForTimeout(500)
    // Wait for drawer to actually be hidden — use ant-drawer-open class
    // (default ant-drawer-content still exists in DOM after close).
    await page.locator('.ant-drawer.ant-drawer-open').waitFor({ state: 'hidden', timeout: 5000 })
  }
}
