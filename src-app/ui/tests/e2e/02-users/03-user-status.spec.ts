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

    // Click the switch
    const userRow = page.locator(`text="${userData.username}"`).locator('..')
    const statusSwitch = userRow
      .locator('button.ant-switch')
      .or(userRow.locator('.ant-switch'))
    await statusSwitch.first().click()

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
    await expect(page.locator('.ant-message-success')).toBeVisible({
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

    // Open reset password drawer
    const resetButton = page
      .getByRole('button', {
        name: new RegExp(`reset password.*${userData.username}`, 'i'),
      })
      .or(
        page
          .locator(`text="${userData.username}"`)
          .locator('..')
          .getByRole('button', { name: /reset password/i })
      )
    await resetButton.first().click()

    // Wait for drawer
    const drawer = page.locator('.ant-drawer:visible', {
      hasText: /reset password/i,
    })
    await drawer.waitFor({ state: 'visible' })

    // Fill in short password
    await page.getByLabel(/new password/i).fill('123') // Less than 6 characters

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
    // Find admin user if it exists
    const adminUser = page.locator('text=admin')
    const adminExists = await adminUser.isVisible()

    if (adminExists) {
      // Check if admin user switch is disabled
      const adminRow = page.locator('text="admin"').locator('..')
      const statusSwitch = adminRow
        .locator('button.ant-switch')
        .or(adminRow.locator('.ant-switch'))

      // Try to click the switch (might be disabled)
      await statusSwitch.first().click({ force: true })

      // If popconfirm appears, try to confirm
      const popconfirm = page.locator('.ant-popconfirm:visible')
      const popconfirmExists = await popconfirm.isVisible()

      if (popconfirmExists) {
        const confirmButton = popconfirm.getByRole('button', { name: /yes/i })
        await confirmButton.click()

        // Should show error message about protected user
        const errorMessage = page.locator('.ant-message-error')
        const errorExists = await errorMessage.isVisible()

        if (errorExists) {
          // If error is shown, admin is protected
          await expect(errorMessage).toBeVisible()
        } else {
          // If no error, check that admin is still active
          await assertUserStatus(page, 'admin', 'active')
        }
      }
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

    // Check active status badge (green/success)
    const userRow = page.locator(`text="${userData.username}"`).locator('..')
    const activeBadge = userRow.locator('.ant-badge-status-success')
    await expect(activeBadge).toBeVisible()

    // Toggle to inactive
    await toggleUserStatus(page, userData.username)

    // Check inactive status badge (red/error)
    const inactiveBadge = userRow.locator('.ant-badge-status-error')
    await expect(inactiveBadge).toBeVisible()
  })
})
