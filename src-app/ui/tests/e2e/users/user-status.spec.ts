import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'
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
    await openCreateUserDrawer(page)
    const timestamp = Date.now()
    const userData = {
      username: `testuser${timestamp}`,
      email: `testuser${timestamp}@example.com`,
      password: 'password123',
    }
    await createUser(page, userData)

    await assertUserStatus(page, userData.username, 'active')
    await toggleUserStatus(page, userData.username)
    await assertUserStatus(page, userData.username, 'inactive')
  })

  test('should toggle user status from inactive to active', async ({
    page,
  }) => {
    await openCreateUserDrawer(page)
    const timestamp = Date.now()
    const userData = {
      username: `testuser${timestamp}`,
      email: `testuser${timestamp}@example.com`,
      password: 'password123',
    }
    await createUser(page, userData)

    await toggleUserStatus(page, userData.username)
    await assertUserStatus(page, userData.username, 'inactive')

    await toggleUserStatus(page, userData.username)
    await assertUserStatus(page, userData.username, 'active')
  })

  test('should require confirmation before toggling status', async ({
    page,
  }) => {
    await openCreateUserDrawer(page)
    const timestamp = Date.now()
    const userData = {
      username: `testuser${timestamp}`,
      email: `testuser${timestamp}@example.com`,
      password: 'password123',
    }
    await createUser(page, userData)

    // Click the row's active switch → the Confirm dialog opens.
    await byTestId(page, `user-row-${userData.username}`)
      .locator('[data-testid^="user-active-switch-"]')
      .click()

    const confirmCancel = page.locator(
      '[data-testid^="user-toggle-active-confirm-"][data-testid$="-cancel"]',
    )
    await expect(confirmCancel).toBeVisible()

    // Cancel → status unchanged.
    await confirmCancel.click()
    await assertUserStatus(page, userData.username, 'active')
  })

  test('should reset user password', async ({ page }) => {
    await openCreateUserDrawer(page)
    const timestamp = Date.now()
    const userData = {
      username: `testuser${timestamp}`,
      email: `testuser${timestamp}@example.com`,
      password: 'password123',
    }
    await createUser(page, userData)

    await resetUserPassword(page, userData.username, 'newpassword456')
    await assertUserExists(page, userData.username)
  })

  test('should show validation error for short password', async ({ page }) => {
    await openCreateUserDrawer(page)
    const timestamp = Date.now()
    const userData = {
      username: `testuser${timestamp}`,
      email: `testuser${timestamp}@example.com`,
      password: 'password123',
    }
    await createUser(page, userData)

    // Open reset password drawer for the user.
    await byTestId(page, `user-row-${userData.username}`)
      .locator('[data-testid^="user-reset-password-button-"]')
      .click()
    await byTestId(page, 'user-reset-password-form').waitFor({
      state: 'visible',
    })

    await byTestId(page, 'user-reset-new-password-input').fill('123') // < 6 chars
    await byTestId(page, 'user-reset-password-submit-button').click()

    await expect(byTestId(page, 'field-error-new_password')).toBeVisible()
  })

  test('should not toggle admin user status (if protected)', async ({
    page,
  }) => {
    const adminRow = byTestId(page, 'user-row-admin')

    if (await adminRow.isVisible()) {
      // The active switch + Delete button are hidden entirely on the root
      // admin row (self / root-admin lockout guards).
      await expect(
        adminRow.locator('[data-testid^="user-active-switch-"]'),
      ).toHaveCount(0)
      await expect(
        adminRow.locator('[data-testid^="user-delete-button-"]'),
      ).toHaveCount(0)

      // The status badge is still present.
      await expect(
        adminRow.locator('[data-testid^="user-status-badge-"]'),
      ).toBeVisible()
    }
  })

  test('should display correct status badge colors', async ({ page }) => {
    await openCreateUserDrawer(page)
    const timestamp = Date.now()
    const userData = {
      username: `testuser${timestamp}`,
      email: `testuser${timestamp}@example.com`,
      password: 'password123',
    }
    await createUser(page, userData)

    // Active by default.
    await assertUserStatus(page, userData.username, 'active')

    await toggleUserStatus(page, userData.username)

    // Inactive after toggle.
    await assertUserStatus(page, userData.username, 'inactive')
  })
})
