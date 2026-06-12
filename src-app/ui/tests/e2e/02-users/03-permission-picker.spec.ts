import { Locator } from '@playwright/test'
import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import {
  navigateToUserGroups,
  openCreateGroupDrawer,
  openEditGroupDrawer,
} from './helpers/user-navigation'
import {
  createGroup,
  enableAdvancedPermissions,
} from './helpers/group-actions'

/**
 * Coverage for the searchable permission picker (dual-mode field that
 * replaced the raw JSON textarea). Two behaviours that the picker MUST
 * guarantee:
 *   1. search -> check -> save round-trips a permission, and
 *   2. wildcard / unknown values not representable in the picker are
 *      preserved verbatim across an edit (the "advanced entries" contract).
 */

// Check a single permission leaf in the picker by its permission string.
async function checkPermissionInPicker(drawer: Locator, permission: string) {
  await drawer.getByPlaceholder('Search permissions').fill(permission)
  // Search auto-expands the matched group; click the leaf's checkbox.
  await drawer
    .locator('.ant-tree-treenode', { hasText: permission })
    .locator('.ant-tree-checkbox')
    .click()
}

// Read the raw permissions array out of the Advanced JSON editor.
async function readAdvancedJson(drawer: Locator) {
  await enableAdvancedPermissions(drawer)
  return drawer.getByLabel(/permissions.*json/i)
}

test.describe('Permission picker', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await navigateToUserGroups(page, baseURL)
  })

  test('search, check, and save a permission via the picker', async ({
    page,
  }) => {
    await openCreateGroupDrawer(page)
    const drawer = page.locator('.ant-drawer.ant-drawer-open')

    const name = `PickerGroup${Date.now()}`
    await drawer.getByLabel(/group name/i).fill(name)

    await checkPermissionInPicker(drawer, 'users::read')

    await drawer.locator('.ant-btn-primary[type="submit"]').click()
    await expect(page.locator('.ant-message-success').first()).toBeVisible({
      timeout: 5000,
    })
    await page.waitForTimeout(500)

    // Reopen and confirm the permission persisted.
    await openEditGroupDrawer(page, name)
    const editDrawer = page.locator('.ant-drawer.ant-drawer-open')
    await expect(await readAdvancedJson(editDrawer)).toHaveValue(/users::read/)
  })

  test('preserves wildcard entries the picker cannot represent', async ({
    page,
  }) => {
    const name = `WildcardGroup${Date.now()}`

    // Seed a group whose only permission is the global wildcard.
    await openCreateGroupDrawer(page)
    await createGroup(page, { name, permissions: ['*'] })

    // Edit in picker mode: the wildcard surfaces as an "advanced entry",
    // and we add a known permission alongside it.
    await openEditGroupDrawer(page, name)
    const drawer = page.locator('.ant-drawer.ant-drawer-open')
    await expect(drawer.getByText(/advanced entr/i)).toBeVisible()

    await checkPermissionInPicker(drawer, 'users::read')

    await drawer.locator('.ant-btn-primary[type="submit"]').click()
    await expect(page.locator('.ant-message-success').first()).toBeVisible({
      timeout: 5000,
    })
    await page.waitForTimeout(500)

    // Reopen: BOTH the wildcard and the newly-checked permission survive.
    await openEditGroupDrawer(page, name)
    const editDrawer = page.locator('.ant-drawer.ant-drawer-open')
    const json = await readAdvancedJson(editDrawer)
    await expect(json).toHaveValue(/users::read/)
    await expect(json).toHaveValue(/"\*"/)
  })
})
