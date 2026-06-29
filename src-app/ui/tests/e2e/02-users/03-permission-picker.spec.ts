import { Page } from '@playwright/test'
import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'
import { loginAsAdmin } from '../../common/auth-helpers'
import {
  navigateToUserGroups,
  openCreateGroupDrawer,
  openEditGroupDrawer,
} from './helpers/user-navigation'
import { createGroup, enableAdvancedPermissions } from './helpers/group-actions'

/**
 * Coverage for the searchable permission picker (dual-mode field that replaced
 * the raw JSON textarea):
 *   1. search -> check -> save round-trips a permission, and
 *   2. wildcard / unknown values not representable in the picker are preserved
 *      verbatim across an edit (the "advanced entries" contract).
 */

// Check a single permission leaf in the picker by its permission string.
// The kit Tree derives `<treeTestid>-check-<key>` for each node's checkbox.
async function checkPermissionInPicker(page: Page, permission: string) {
  await byTestId(page, 'user-permissions-search-input').fill(permission)
  await byTestId(page, `user-permissions-tree-check-${permission}`).click()
}

// Flip to the Advanced JSON editor and return the raw-array textarea locator.
function readAdvancedJson(page: Page) {
  return enableAdvancedPermissions(page).then(() =>
    byTestId(page, 'user-permissions-json-textarea'),
  )
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

    const name = `PickerGroup${Date.now()}`
    await byTestId(page, 'user-create-group-name-input').fill(name)

    await checkPermissionInPicker(page, 'users::read')

    await byTestId(page, 'user-create-group-submit-button').click()
    await expect(
      page.locator('[data-sonner-toast][data-type="success"]').first(),
    ).toBeVisible({ timeout: 5000 })

    // Reopen and confirm the permission persisted.
    await openEditGroupDrawer(page, name)
    await expect(await readAdvancedJson(page)).toHaveValue(/users::read/)
  })

  test('preserves wildcard entries the picker cannot represent', async ({
    page,
  }) => {
    const name = `WildcardGroup${Date.now()}`

    // Seed a group whose only permission is the global wildcard.
    await openCreateGroupDrawer(page)
    await createGroup(page, { name, permissions: ['*'] })

    // Edit in picker mode: the wildcard surfaces as an "advanced entry".
    await openEditGroupDrawer(page, name)
    await expect(byTestId(page, 'user-permissions-extra-note')).toBeVisible()

    await checkPermissionInPicker(page, 'users::read')

    await byTestId(page, 'user-edit-group-save-button').click()
    await expect(
      page.locator('[data-sonner-toast][data-type="success"]').first(),
    ).toBeVisible({ timeout: 5000 })

    // Reopen: BOTH the wildcard and the newly-checked permission survive.
    await openEditGroupDrawer(page, name)
    const json = await readAdvancedJson(page)
    await expect(json).toHaveValue(/users::read/)
    await expect(json).toHaveValue(/"\*"/)
  })
})
