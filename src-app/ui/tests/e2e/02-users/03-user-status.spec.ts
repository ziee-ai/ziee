import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import {
  navigateToUsers,
  openCreateUserDrawer,
} from './helpers/user-navigation'
import {
  createUser,
  toggleUserStatus,
  resetUserPassword,
} from './helpers/user-actions'
import { assertUserExists, assertUserStatus } from './helpers/user-assertions'

test.describe('User Status Management', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await navigateToUsers(page, baseURL)
  })

  test('should toggle user status from active to inactive', async ({
    page,
  }) => {
    // Create a user first
    await openCreateUserDrawer(page)
    const timestamp = Date.now()
    const userData = {
      username: `testuser${timestamp}`,
      email: `testuser${timestamp}@example.com`,
      password: 'password123',
    }
    await createUser(page, userData)

    // Verify user is initially active
    await assertUserStatus(page, userData.username, 'active')

    // Toggle status to inactive
    await toggleUserStatus(page, userData.username)

    // Verify user is now inactive
    await assertUserStatus(page, userData.username, 'inactive')
  })

  test('should toggle user status from inactive to active', async ({
    page,
  }) => {
    // Create a user
    await openCreateUserDrawer(page)
    const timestamp = Date.now()
    const userData = {
      username: `testuser${timestamp}`,
      email: `testuser${timestamp}@example.com`,
      password: 'password123',
    }
    await createUser(page, userData)

    // Toggle to inactive
    await toggleUserStatus(page, userData.username)
    await assertUserStatus(page, userData.username, 'inactive')

    // Toggle back to active
    await toggleUserStatus(page, userData.username)
    await assertUserStatus(page, userData.username, 'active')
  })

  test('should require confirmation before toggling status', async ({
    page,
  }) => {
    // Create a user
    await openCreateUserDrawer(page)
    const timestamp = Date.now()
    const userData = {
      username: `testuser${timestamp}`,
      email: `testuser${timestamp}@example.com`,
      password: 'password123',
    }
    await createUser(page, userData)

    // Click the switch — anchor on the typography span and walk up 3
    // levels to the row container (same as toggleUserStatus helper).
    const usernameEl = page
      .locator('.ant-typography.font-medium', { hasText: userData.username })
      .first()
    const userRow = usernameEl.locator('xpath=ancestor::div[contains(@class, "mb-2")][1]')
    const statusSwitch = userRow.locator('button.ant-switch').first()
    await statusSwitch.click()

    // Verify popconfirm appears
    const popconfirm = page.locator('.ant-popconfirm:visible')
    await expect(popconfirm).toBeVisible()

    // Verify confirmation buttons exist
    await expect(
      popconfirm.getByRole('button', { name: /yes/i })
    ).toBeVisible()
    await expect(
      popconfirm.getByRole('button', { name: /no/i })
    ).toBeVisible()

    // Cancel the action
    await popconfirm.getByRole('button', { name: /no/i }).click()

    // Verify status didn't change
    await assertUserStatus(page, userData.username, 'active')
  })

  test('should reset user password', async ({ page }) => {
    // Create a user
    await openCreateUserDrawer(page)
    const timestamp = Date.now()
    const userData = {
      username: `testuser${timestamp}`,
      email: `testuser${timestamp}@example.com`,
      password: 'password123',
    }
    await createUser(page, userData)

    // Reset password
    const newPassword = 'newpassword456'
    await resetUserPassword(page, userData.username, newPassword)

    // Verify success message
    await expect(page.locator('.ant-message-success').first()).toBeVisible({
      timeout: 5000,
    })

    // Verify user still exists
    await assertUserExists(page, userData.username)
  })

  test('should show validation error for short password', async ({ page }) => {
    // Create a user
    await openCreateUserDrawer(page)
    const timestamp = Date.now()
    const userData = {
      username: `testuser${timestamp}`,
      email: `testuser${timestamp}@example.com`,
      password: 'password123',
    }
    await createUser(page, userData)

    // Open reset password drawer (anchor on username span and walk
    // up 3 levels to the row container; one-level-up missed the
    // sibling button section).
    const usernameEl = page
      .locator('.ant-typography.font-medium', { hasText: userData.username })
      .first()
    const userRow = usernameEl.locator('xpath=ancestor::div[contains(@class, "mb-2")][1]')
    const resetButton = page
      .getByRole('button', {
        name: new RegExp(`reset password.*${userData.username}`, 'i'),
      })
      .or(userRow.getByRole('button', { name: /reset password/i }))
    await resetButton.first().click()

    // Wait for drawer
    const drawer = page.locator('.ant-drawer.ant-drawer-open', {
      hasText: /reset password/i,
    })
    await drawer.waitFor({ state: 'visible' })

    // Fill in short password (scope to drawer)
    await drawer.getByLabel(/new password/i).fill('123') // Less than 6 characters

    // Try to submit
    const submitButton = drawer.getByRole('button', {
      name: /reset password/i,
    })
    await submitButton.click()

    // Check for validation error
    await expect(
      page.locator('.ant-form-item-explain-error', {
        hasText: /at least 6 characters/i,
      })
    ).toBeVisible()
  })

  test('should not toggle admin user status (if protected)', async ({
    page,
  }) => {
    // Find admin user if it exists (use the typography span to avoid
    // matching the nav menu or breadcrumb).
    const adminUser = page
      .locator('.ant-typography.font-medium', { hasText: 'admin' })
      .first()
    const adminExists = await adminUser.isVisible()

    if (adminExists) {
      // Walk up 3 levels to the row container (same shape as
      // toggleUserStatus helper).
      const adminRow = adminUser.locator('xpath=ancestor::div[contains(@class, "mb-2")][1]')

      // The active-status Switch and the Delete button are hidden
      // entirely on the root admin row (UsersSettings.tsx self/
      // root-admin lockout guards — audit 03 B-6). The backend also
      // rejects toggling root admin to inactive, but the UI is the
      // first line of defense and should never offer the control.
      await expect(adminRow.locator('.ant-switch')).toHaveCount(0)
      await expect(
        adminRow.getByRole('button', { name: /delete/i }),
      ).toHaveCount(0)

      // The badge inside this row should still read 'Active' (or
      // 'Inactive' — point is the badge is present even though the
      // toggle is hidden).
      await expect(
        adminRow.locator('.ant-badge-status-text').first(),
      ).toBeVisible()
    }
  })

  test('should display correct status badge colors', async ({ page }) => {
    // Create a user
    await openCreateUserDrawer(page)
    const timestamp = Date.now()
    const userData = {
      username: `testuser${timestamp}`,
      email: `testuser${timestamp}@example.com`,
      password: 'password123',
    }
    await createUser(page, userData)

    // Check active status badge (green/success) — anchor on
    // typography span and walk up 3 levels to the row container.
    const usernameEl = page
      .locator('.ant-typography.font-medium', { hasText: userData.username })
      .first()
    const userRow = usernameEl.locator('xpath=ancestor::div[contains(@class, "mb-2")][1]')
    const activeBadge = userRow.locator('.ant-badge-status-success').first()
    await expect(activeBadge).toBeVisible()

    // Toggle to inactive
    await toggleUserStatus(page, userData.username)

    // Check inactive status badge (red/error)
    const inactiveBadge = userRow.locator('.ant-badge-status-error').first()
    await expect(inactiveBadge).toBeVisible()
  })
})
