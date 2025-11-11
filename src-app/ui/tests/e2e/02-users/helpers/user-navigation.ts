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
 */
export async function openCreateUserDrawer(page: Page) {
  const createButton = page.getByRole('button', { name: /create user/i })
  await createButton.click()

  // Wait for drawer to appear
  const drawer = page.locator('.ant-drawer:visible', { hasText: 'Create User' })
  await drawer.waitFor({ state: 'visible' })
}

/**
 * Open the create group drawer
 */
export async function openCreateGroupDrawer(page: Page) {
  const createButton = page.getByRole('button', { name: /create group/i })
  await createButton.click()

  // Wait for drawer to appear
  const drawer = page.locator('.ant-drawer:visible', { hasText: 'Create User Group' })
  await drawer.waitFor({ state: 'visible' })
}

/**
 * Open the edit user drawer for a specific user
 */
export async function openEditUserDrawer(page: Page, username: string) {
  // Find username element, go up to user section, then find Edit button
  const usernameEl = page.locator('.ant-typography.font-medium', { hasText: username }).first()

  // Go up to the user info section container (2 levels up from username text)
  const userSection = usernameEl.locator('../..')

  // Find Edit button within that section
  const editButton = userSection.getByRole('button', { name: /^edit$/i })
  await editButton.waitFor({ state: 'visible', timeout: 5000 })
  await editButton.click()

  // Wait for drawer to appear
  const drawer = page.locator('.ant-drawer:visible', { hasText: /edit user/i })
  await drawer.waitFor({ state: 'visible' })
}

/**
 * Open the edit group drawer for a specific group
 */
export async function openEditGroupDrawer(page: Page, groupName: string) {
  const editButton = page.getByRole('button', { name: new RegExp(`edit.*${groupName}`, 'i') })
    .or(page.locator(`text="${groupName}"`).locator('..').getByRole('button', { name: /edit/i }))

  await editButton.first().click()

  // Wait for drawer to appear
  const drawer = page.locator('.ant-drawer:visible', { hasText: /edit group/i })
  await drawer.waitFor({ state: 'visible' })
}

/**
 * Open the groups drawer for a specific user
 */
export async function openUserGroupsDrawer(page: Page, username: string) {
  const groupsButton = page.getByRole('button', { name: new RegExp(`groups.*${username}`, 'i') })
    .or(page.locator(`text="${username}"`).locator('..').getByRole('button', { name: /groups/i }))

  await groupsButton.first().click()

  // Wait for drawer to appear
  const drawer = page.locator('.ant-drawer:visible')
  await drawer.waitFor({ state: 'visible' })
}

/**
 * Open the members drawer for a specific group
 */
export async function openGroupMembersDrawer(page: Page, groupName: string) {
  const membersButton = page.getByRole('button', { name: new RegExp(`members.*${groupName}`, 'i') })
    .or(page.locator(`text="${groupName}"`).locator('..').getByRole('button', { name: /members/i }))

  await membersButton.first().click()

  // Wait for drawer to appear
  const drawer = page.locator('.ant-drawer:visible', { hasText: /members of/i })
  await drawer.waitFor({ state: 'visible' })
}

/**
 * Close any open drawer
 */
export async function closeDrawer(page: Page) {
  const closeButton = page.locator('.ant-drawer:visible .ant-drawer-close')
  if (await closeButton.isVisible()) {
    await closeButton.click()
    // Wait for drawer animation to complete
    await page.waitForTimeout(500)
    // Wait for drawer to actually be hidden
    await page.locator('.ant-drawer-content').waitFor({ state: 'hidden', timeout: 5000 })
  }
}
