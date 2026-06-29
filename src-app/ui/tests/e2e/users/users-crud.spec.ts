import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'
import { loginAsAdmin } from '../../common/auth-helpers'
import {
  navigateToUsers,
  openCreateUserDrawer,
  openEditUserDrawer,
  closeDrawer,
} from './helpers/user-navigation'
import {
  createUser,
  updateUser,
  deleteUser,
  expectErrorToast,
} from './helpers/user-actions'
import {
  assertUserExists,
  assertUserNotExists,
  assertUserStatus,
  assertDrawerOpen,
  assertDrawerClosed,
} from './helpers/user-assertions'

test.describe('Users CRUD Operations', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await navigateToUsers(page, baseURL)
  })

  test('should display users list page', async ({ page }) => {
    await expect(byTestId(page, 'user-list-card')).toBeVisible()
    await expect(byTestId(page, 'user-create-open-button')).toBeVisible()
  })

  test('should create a new user', async ({ page }) => {
    await openCreateUserDrawer(page)
    await assertDrawerOpen(page)

    const timestamp = Date.now()
    const userData = {
      username: `testuser${timestamp}`,
      email: `testuser${timestamp}@example.com`,
      password: 'password123',
      displayName: 'Test User',
    }

    await createUser(page, userData)
    await assertDrawerClosed(page)
    await assertUserExists(page, userData.username)
  })

  test('should create user with permissions', async ({ page }) => {
    await openCreateUserDrawer(page)

    const timestamp = Date.now()
    const userData = {
      username: `testuser${timestamp}`,
      email: `testuser${timestamp}@example.com`,
      password: 'password123',
      permissions: ['users::read', 'groups::read'],
    }

    await createUser(page, userData)
    await assertUserExists(page, userData.username)
  })

  test('should show validation errors for invalid user data', async ({
    page,
  }) => {
    await openCreateUserDrawer(page)

    // Submit without required fields → the username field surfaces an error.
    await byTestId(page, 'user-create-submit-button').click()
    await expect(byTestId(page, 'field-error-username')).toBeVisible()
  })

  test('should show error for duplicate username', async ({ page }) => {
    await openCreateUserDrawer(page)
    const timestamp = Date.now()
    const userData = {
      username: `testuser${timestamp}`,
      email: `testuser${timestamp}@example.com`,
      password: 'password123',
    }
    await createUser(page, userData)

    // Try to create user with the same username.
    await openCreateUserDrawer(page)
    await byTestId(page, 'user-create-username-input').fill(userData.username)
    await byTestId(page, 'user-create-email-input').fill(
      `different${timestamp}@example.com`,
    )
    await byTestId(page, 'user-create-password-input').fill('password123')

    await byTestId(page, 'user-create-submit-button').click()
    await expectErrorToast(page)
  })

  test('should edit an existing user', async ({ page }) => {
    await openCreateUserDrawer(page)
    const timestamp = Date.now()
    const userData = {
      username: `testuser${timestamp}`,
      email: `testuser${timestamp}@example.com`,
      password: 'password123',
    }
    await createUser(page, userData)

    await openEditUserDrawer(page, userData.username)
    await assertDrawerOpen(page)

    await updateUser(page, {
      email: `updated${timestamp}@example.com`,
      displayName: 'Updated User',
    })

    await assertDrawerClosed(page)
    await assertUserExists(page, userData.username)
  })

  test('edit drawer Active switch deactivates a user', async ({ page }) => {
    await openCreateUserDrawer(page)
    const timestamp = Date.now()
    const userData = {
      username: `inactuser${timestamp}`,
      email: `inactuser${timestamp}@example.com`,
      password: 'password123',
    }
    await createUser(page, userData)
    await assertUserStatus(page, userData.username, 'active')

    await openEditUserDrawer(page, userData.username)
    await assertDrawerOpen(page)

    const activeSwitch = byTestId(page, 'user-edit-active-switch')
    await expect(activeSwitch).toHaveAttribute('aria-checked', 'true')
    await activeSwitch.click()
    await expect(activeSwitch).toHaveAttribute('aria-checked', 'false')

    await byTestId(page, 'user-edit-submit-button').click()
    await assertDrawerClosed(page)

    await assertUserStatus(page, userData.username, 'inactive')
  })

  test('should cancel user creation', async ({ page }) => {
    await openCreateUserDrawer(page)
    await assertDrawerOpen(page)

    await byTestId(page, 'user-create-username-input').fill('canceltest')

    await byTestId(page, 'user-create-cancel-button').click()
    await assertDrawerClosed(page)

    await assertUserNotExists(page, 'canceltest')
  })

  test('should close drawer with close button', async ({ page }) => {
    await openCreateUserDrawer(page)
    await assertDrawerOpen(page)

    await closeDrawer(page)
    await assertDrawerClosed(page)
  })

  test('should delete a user', async ({ page }) => {
    await openCreateUserDrawer(page)
    const timestamp = Date.now()
    const userData = {
      username: `testuser${timestamp}`,
      email: `testuser${timestamp}@example.com`,
      password: 'password123',
    }
    await createUser(page, userData)

    await assertUserExists(page, userData.username)
    await deleteUser(page, userData.username)
    await assertUserNotExists(page, userData.username)
  })

  test('should validate password minimum length', async ({ page }) => {
    await openCreateUserDrawer(page)

    const timestamp = Date.now()
    await byTestId(page, 'user-create-username-input').fill(
      `testuser${timestamp}`,
    )
    await byTestId(page, 'user-create-email-input').fill(
      `testuser${timestamp}@example.com`,
    )
    await byTestId(page, 'user-create-password-input').fill('123') // < 6 chars

    await byTestId(page, 'user-create-submit-button').click()

    await expect(byTestId(page, 'field-error-password')).toBeVisible()
  })

  test('should validate email format', async ({ page }) => {
    await openCreateUserDrawer(page)

    const timestamp = Date.now()
    await byTestId(page, 'user-create-username-input').fill(
      `testuser${timestamp}`,
    )
    await byTestId(page, 'user-create-email-input').fill('invalid-email')
    await byTestId(page, 'user-create-password-input').fill('password123')

    await byTestId(page, 'user-create-submit-button').click()

    await expect(byTestId(page, 'field-error-email')).toBeVisible()
  })

  test('should handle pagination', async ({ page }) => {
    const pagination = byTestId(page, 'user-list-pagination')
    await expect(pagination).toBeVisible()
    // Total summary reports the user count ("…of N users").
    await expect(pagination).toContainText('of')

    // Change the page size via the size-changer Select.
    const sizeSelect = byTestId(page, 'user-list-pagination-page-size')
    if (await sizeSelect.isVisible()) {
      await sizeSelect.click()
      await byTestId(page, 'user-list-pagination-page-size-opt-20').click()
      await expect(pagination).toBeVisible()
    }
  })
})
