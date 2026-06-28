import { test, expect } from '../../fixtures/test-context'
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
    // Check page title — the page renders an h4 (level=4), not h1.
    await expect(
      page.getByRole('heading', { name: /user groups/i, level: 4 })
    ).toBeVisible()

    // Check create group button exists (multiple matching buttons may
    // exist if a closed drawer's submit button is still in the DOM —
    // `.first()` grabs the trigger in document order).
    await expect(
      page.getByRole('button', { name: /create group/i }).first()
    ).toBeVisible()
  })

  test('should create a new group', async ({ page }) => {
    // Open create group drawer
    await openCreateGroupDrawer(page)
    await assertDrawerOpen(page, 'Create User Group')

    // Create group
    const timestamp = Date.now()
    const groupData = {
      name: `TestGroup${timestamp}`,
      description: 'Test group description',
    }

    await createGroup(page, groupData)

    // Verify drawer closed
    await assertDrawerClosed(page)

    // Verify group appears in list
    await assertGroupExists(page, groupData.name)
  })

  test('should create group with permissions', async ({ page }) => {
    // Open create group drawer
    await openCreateGroupDrawer(page)

    // Create group with permissions
    const timestamp = Date.now()
    const groupData = {
      name: `TestGroup${timestamp}`,
      description: 'Test group with permissions',
      permissions: ['users::read', 'users::create', 'groups::read'],
    }

    await createGroup(page, groupData)

    // Verify group appears in list
    await assertGroupExists(page, groupData.name)
  })

  test('should show validation errors for invalid group data', async ({
    page,
  }) => {
    await openCreateGroupDrawer(page)

    // Try to submit without required fields
    // Drawer submit label was standardised to "Create" (audit I-2);
    // scope by primary-button class to avoid colliding with the list
    // CTA which still carries aria-label="Create group".
    const submitButton = page
      .locator('.ant-drawer.ant-drawer-open .ant-btn-primary[type="submit"]')
    await submitButton.click()

    // Check for validation errors
    await expect(
      page.locator('.ant-form-item-explain-error').first()
    ).toBeVisible()
  })

  test('should show error for invalid permissions JSON', async ({ page }) => {
    await openCreateGroupDrawer(page)
    const drawer = page.locator('.ant-drawer.ant-drawer-open')

    const timestamp = Date.now()
    await drawer.getByLabel(/group name/i).fill(`TestGroup${timestamp}`)

    // The permissions field defaults to the picker; validation of raw
    // arrays lives in the Advanced JSON editor, which surfaces an inline
    // role="alert" error rather than a Form.Item explain message.
    await enableAdvancedPermissions(drawer)
    await drawer.getByLabel(/permissions.*json/i).fill('invalid json')

    await expect(drawer.getByText(/invalid json format/i)).toBeVisible()
  })

  test('should show error for invalid permission values', async ({ page }) => {
    await openCreateGroupDrawer(page)
    const drawer = page.locator('.ant-drawer.ant-drawer-open')

    const timestamp = Date.now()
    await drawer.getByLabel(/group name/i).fill(`TestGroup${timestamp}`)

    await enableAdvancedPermissions(drawer)
    await drawer
      .getByLabel(/permissions.*json/i)
      .fill('["invalid::permission"]') // valid JSON, unknown permission

    await expect(drawer.getByText(/invalid permissions/i)).toBeVisible()
  })

  test('should edit an existing group', async ({ page }) => {
    // Create a group first
    await openCreateGroupDrawer(page)
    const timestamp = Date.now()
    const groupData = {
      name: `TestGroup${timestamp}`,
      description: 'Original description',
    }
    await createGroup(page, groupData)

    // Open edit drawer
    await openEditGroupDrawer(page, groupData.name)
    await assertDrawerOpen(page, /edit group/i)

    // Update group
    const updatedData = {
      description: 'Updated description',
      permissions: ['users::read'],
    }
    await updateGroup(page, updatedData)

    // Verify drawer closed
    await assertDrawerClosed(page)

    // Verify group still exists
    await assertGroupExists(page, groupData.name)
  })

  test('should toggle group active status', async ({ page }) => {
    // Create a group first
    await openCreateGroupDrawer(page)
    const timestamp = Date.now()
    const groupData = {
      name: `TestGroup${timestamp}`,
      description: 'Test group',
    }
    await createGroup(page, groupData)

    // Verify initial status (active by default)
    await assertGroupStatus(page, groupData.name, 'active')

    // Open edit drawer and toggle status
    await openEditGroupDrawer(page, groupData.name)
    await updateGroup(page, { isActive: false })

    // Verify status changed to inactive
    await assertGroupStatus(page, groupData.name, 'inactive')

    // Toggle back to active
    await openEditGroupDrawer(page, groupData.name)
    await updateGroup(page, { isActive: true })

    // Verify status changed to active
    await assertGroupStatus(page, groupData.name, 'active')
  })

  test('should cancel group creation', async ({ page }) => {
    await openCreateGroupDrawer(page)
    await assertDrawerOpen(page, 'Create User Group')

    // Fill some data
    await page.getByLabel(/group name/i).fill('CancelGroup')

    // Click cancel button
    const cancelButton = page
      .locator('.ant-drawer.ant-drawer-open')
      .getByRole('button', { name: /cancel/i })
    await cancelButton.click()

    // Verify drawer closed
    await assertDrawerClosed(page)

    // Verify group was not created
    await assertGroupNotExists(page, 'CancelGroup')
  })

  test('should close drawer with close button', async ({ page }) => {
    await openCreateGroupDrawer(page)
    await assertDrawerOpen(page, 'Create User Group')

    // Close drawer
    await closeDrawer(page)

    // Verify drawer closed
    await assertDrawerClosed(page)
  })

  test('should delete a non-system group', async ({ page }) => {
    // Create a group first
    await openCreateGroupDrawer(page)
    const timestamp = Date.now()
    const groupData = {
      name: `TestGroup${timestamp}`,
      description: 'Test group to delete',
    }
    await createGroup(page, groupData)

    // Verify group exists
    await assertGroupExists(page, groupData.name)

    // Delete group
    await deleteGroup(page, groupData.name)

    // Verify group no longer exists
    await assertGroupNotExists(page, groupData.name)
  })

  test('cancelling the delete popconfirm keeps the group', async ({ page }) => {
    // Create a group to (not) delete.
    await openCreateGroupDrawer(page)
    const groupData = {
      name: `CancelDel${Date.now()}`,
      description: 'Should survive a cancelled delete',
    }
    await createGroup(page, groupData)
    await assertGroupExists(page, groupData.name)

    // Open the delete popconfirm, then CANCEL instead of confirming.
    const deleteButton = page
      .getByRole('button', {
        name: new RegExp(`delete.*${groupData.name}`, 'i'),
      })
      .or(
        page
          .locator(`text="${groupData.name}"`)
          .first()
          .locator('../..')
          .getByRole('button', { name: /delete/i }),
      )
    await deleteButton.first().click()

    const popconfirm = page.locator('.ant-popconfirm:visible')
    await expect(popconfirm).toBeVisible()
    // The cancel/secondary button (NOT the primary confirm).
    await popconfirm.locator('.ant-btn:not(.ant-btn-primary)').first().click()

    // The popconfirm closes and the group is still present (no deletion).
    await expect(page.locator('.ant-popconfirm:visible')).toHaveCount(0)
    await assertGroupExists(page, groupData.name)
  })

  test('should show system tag for system groups', async ({ page }) => {
    // Check if there are any system groups in the list
    const systemTag = page.locator('.ant-tag', { hasText: /system/i }).first()
    const hasSystemGroups = await systemTag.isVisible()

    if (hasSystemGroups) {
      // Verify system tag is visible
      await expect(systemTag).toBeVisible()

      // System groups typically include admin group
      const adminGroup = page.locator('.ant-card', { hasText: /admin/i }).first()
      if (await adminGroup.isVisible()) {
        const adminSystemTag = adminGroup.locator('.ant-tag', {
          hasText: /system/i,
        }).first()
        await expect(adminSystemTag).toBeVisible()
      }
    }
  })

  test('should handle pagination', async ({ page }) => {
    // Check if pagination exists
    const pagination = page.locator('.ant-pagination')
    const paginationExists = await pagination.isVisible()

    if (paginationExists) {
      // Get current page info
      const totalText = await page
        .locator('.ant-pagination-total-text')
        .textContent()
      expect(totalText).toContain('of')
      expect(totalText).toContain('groups')

      // Try to change page size if selector is available
      const pageSizeSelector = page.locator('.ant-select-selector', {
        has: page.locator('span', { hasText: /\d+ \/ page/i }),
      })
      if (await pageSizeSelector.first().isVisible()) {
        await pageSizeSelector.first().click()
        await page
          .locator('.ant-select-dropdown:visible')
          .getByText('20')
          .click()
        await page.waitForTimeout(500)

        // Verify page updated
        await expect(
          page.locator('.ant-pagination-total-text')
        ).toBeVisible()
      }
    }
  })

  test('should update group name', async ({ page }) => {
    // Create a group first
    await openCreateGroupDrawer(page)
    const timestamp = Date.now()
    const groupData = {
      name: `TestGroup${timestamp}`,
      description: 'Test group',
    }
    await createGroup(page, groupData)

    // Open edit drawer
    await openEditGroupDrawer(page, groupData.name)

    // Update group name
    const newName = `UpdatedGroup${timestamp}`
    await updateGroup(page, { name: newName })

    // Verify old name doesn't exist
    await assertGroupNotExists(page, groupData.name)

    // Verify new name exists
    await assertGroupExists(page, newName)
  })

  test('should clear description field', async ({ page }) => {
    // Create a group with description
    await openCreateGroupDrawer(page)
    const timestamp = Date.now()
    const groupData = {
      name: `TestGroup${timestamp}`,
      description: 'Original description',
    }
    await createGroup(page, groupData)

    // Open edit drawer
    await openEditGroupDrawer(page, groupData.name)

    // Clear description
    await updateGroup(page, { description: '' })

    // Verify group still exists
    await assertGroupExists(page, groupData.name)
  })
})
