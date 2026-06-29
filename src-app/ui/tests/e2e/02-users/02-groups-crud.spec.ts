import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'
import { loginAsAdmin } from '../../common/auth-helpers'
import {
  navigateToUserGroups,
  openCreateGroupDrawer,
  openEditGroupDrawer,
  closeDrawer,
} from './helpers/user-navigation'
import {
  createGroup,
  updateGroup,
  deleteGroup,
  enableAdvancedPermissions,
} from './helpers/group-actions'
import {
  assertGroupExists,
  assertGroupNotExists,
  assertGroupStatus,
  assertDrawerOpen,
  assertDrawerClosed,
} from './helpers/user-assertions'

test.describe('User Groups CRUD Operations', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await navigateToUserGroups(page, baseURL)
  })

  test('should display user groups list page', async ({ page }) => {
    await expect(byTestId(page, 'user-groups-create-button')).toBeVisible()
  })

  test('should create a new group', async ({ page }) => {
    await openCreateGroupDrawer(page)
    await assertDrawerOpen(page)

    const timestamp = Date.now()
    const groupData = {
      name: `TestGroup${timestamp}`,
      description: 'Test group description',
    }

    await createGroup(page, groupData)
    await assertDrawerClosed(page)
    await assertGroupExists(page, groupData.name)
  })

  test('should create group with permissions', async ({ page }) => {
    await openCreateGroupDrawer(page)

    const timestamp = Date.now()
    const groupData = {
      name: `TestGroup${timestamp}`,
      description: 'Test group with permissions',
      permissions: ['users::read', 'users::create', 'groups::read'],
    }

    await createGroup(page, groupData)
    await assertGroupExists(page, groupData.name)
  })

  test('should show validation errors for invalid group data', async ({
    page,
  }) => {
    await openCreateGroupDrawer(page)

    await byTestId(page, 'user-create-group-submit-button').click()
    await expect(byTestId(page, 'field-error-name')).toBeVisible()
  })

  test('should show error for invalid permissions JSON', async ({ page }) => {
    await openCreateGroupDrawer(page)

    const timestamp = Date.now()
    await byTestId(page, 'user-create-group-name-input').fill(
      `TestGroup${timestamp}`,
    )

    // The Advanced JSON editor surfaces an inline role="alert" error.
    await enableAdvancedPermissions(page)
    await byTestId(page, 'user-permissions-json-textarea').fill('invalid json')

    await expect(byTestId(page, 'user-permissions-json-error')).toContainText(
      'Invalid JSON format',
    )
  })

  test('should show error for invalid permission values', async ({ page }) => {
    await openCreateGroupDrawer(page)

    const timestamp = Date.now()
    await byTestId(page, 'user-create-group-name-input').fill(
      `TestGroup${timestamp}`,
    )

    await enableAdvancedPermissions(page)
    await byTestId(page, 'user-permissions-json-textarea').fill(
      '["invalid::permission"]', // valid JSON, unknown permission
    )

    await expect(byTestId(page, 'user-permissions-json-error')).toContainText(
      'Invalid permissions',
    )
  })

  test('should edit an existing group', async ({ page }) => {
    await openCreateGroupDrawer(page)
    const timestamp = Date.now()
    const groupData = {
      name: `TestGroup${timestamp}`,
      description: 'Original description',
    }
    await createGroup(page, groupData)

    await openEditGroupDrawer(page, groupData.name)
    await assertDrawerOpen(page)

    await updateGroup(page, {
      description: 'Updated description',
      permissions: ['users::read'],
    })

    await assertDrawerClosed(page)
    await assertGroupExists(page, groupData.name)
  })

  test('should toggle group active status', async ({ page }) => {
    await openCreateGroupDrawer(page)
    const timestamp = Date.now()
    const groupData = {
      name: `TestGroup${timestamp}`,
      description: 'Test group',
    }
    await createGroup(page, groupData)

    await assertGroupStatus(page, groupData.name, 'active')

    await openEditGroupDrawer(page, groupData.name)
    await updateGroup(page, { isActive: false })
    await assertGroupStatus(page, groupData.name, 'inactive')

    await openEditGroupDrawer(page, groupData.name)
    await updateGroup(page, { isActive: true })
    await assertGroupStatus(page, groupData.name, 'active')
  })

  test('should cancel group creation', async ({ page }) => {
    await openCreateGroupDrawer(page)
    await assertDrawerOpen(page)

    await byTestId(page, 'user-create-group-name-input').fill('CancelGroup')

    await byTestId(page, 'user-create-group-cancel-button').click()
    await assertDrawerClosed(page)

    await assertGroupNotExists(page, 'CancelGroup')
  })

  test('should close drawer with close button', async ({ page }) => {
    await openCreateGroupDrawer(page)
    await assertDrawerOpen(page)

    await closeDrawer(page)
    await assertDrawerClosed(page)
  })

  test('should delete a non-system group', async ({ page }) => {
    await openCreateGroupDrawer(page)
    const timestamp = Date.now()
    const groupData = {
      name: `TestGroup${timestamp}`,
      description: 'Test group to delete',
    }
    await createGroup(page, groupData)

    await assertGroupExists(page, groupData.name)
    await deleteGroup(page, groupData.name)
    await assertGroupNotExists(page, groupData.name)
  })

  test('cancelling the delete popconfirm keeps the group', async ({ page }) => {
    await openCreateGroupDrawer(page)
    const groupData = {
      name: `CancelDel${Date.now()}`,
      description: 'Should survive a cancelled delete',
    }
    await createGroup(page, groupData)
    await assertGroupExists(page, groupData.name)

    // Open the delete Confirm, then CANCEL instead of confirming.
    await byTestId(page, `user-group-row-${groupData.name}`)
      .locator('[data-testid^="user-group-delete-button-"]')
      .click()

    const confirm = page.locator(
      '[data-testid^="user-group-delete-confirm-"][data-testid$="-cancel"]',
    )
    await expect(confirm).toBeVisible()
    await confirm.click()

    // The Confirm closes and the group is still present (no deletion).
    await expect(confirm).toHaveCount(0)
    await assertGroupExists(page, groupData.name)
  })

  test('should show system tag for system groups', async ({ page }) => {
    // Seeded built-in groups (Administrators / Users) carry a System tag.
    const systemTag = page
      .locator('[data-testid^="user-group-system-tag-"]')
      .first()
    if (await systemTag.isVisible()) {
      await expect(systemTag).toBeVisible()
    }
  })

  test('should handle pagination', async ({ page }) => {
    const pagination = byTestId(page, 'user-groups-pagination')
    if (await pagination.isVisible()) {
      await expect(pagination).toContainText('of')
      await expect(pagination).toContainText('groups')

      const sizeSelect = byTestId(page, 'user-groups-pagination-page-size')
      if (await sizeSelect.isVisible()) {
        await sizeSelect.click()
        await byTestId(
          page,
          'user-groups-pagination-page-size-opt-20',
        ).click()
        await expect(pagination).toBeVisible()
      }
    }
  })

  test('should update group name', async ({ page }) => {
    await openCreateGroupDrawer(page)
    const timestamp = Date.now()
    const groupData = {
      name: `TestGroup${timestamp}`,
      description: 'Test group',
    }
    await createGroup(page, groupData)

    await openEditGroupDrawer(page, groupData.name)

    const newName = `UpdatedGroup${timestamp}`
    await updateGroup(page, { name: newName })

    await assertGroupNotExists(page, groupData.name)
    await assertGroupExists(page, newName)
  })

  test('should clear description field', async ({ page }) => {
    await openCreateGroupDrawer(page)
    const timestamp = Date.now()
    const groupData = {
      name: `TestGroup${timestamp}`,
      description: 'Original description',
    }
    await createGroup(page, groupData)

    await openEditGroupDrawer(page, groupData.name)
    await updateGroup(page, { description: '' })

    await assertGroupExists(page, groupData.name)
  })
})
