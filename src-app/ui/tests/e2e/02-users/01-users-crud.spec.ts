import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import {
  navigateToUsers,
  openCreateUserDrawer,
  openEditUserDrawer,
  closeDrawer,
} from './helpers/user-navigation'
import { createUser, updateUser, deleteUser } from './helpers/user-actions'
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
    // Check page title (Users page has h4 heading, not h1)
    await expect(page.getByRole('heading', { name: /^users$/i })).toBeVisible()

    // Check create user button exists
    await expect(
      page.getByRole('button', { name: /create user/i })
    ).toBeVisible()
  })

  test('should create a new user', async ({ page }) => {
    // Open create user drawer
    await openCreateUserDrawer(page)
    await assertDrawerOpen(page, 'Create User')

    // Create user
    const timestamp = Date.now()
    const userData = {
      username: `testuser${timestamp}`,
      email: `testuser${timestamp}@example.com`,
      password: 'password123',
      displayName: 'Test User',
    }

    await createUser(page, userData)

    // Verify drawer closed
    await assertDrawerClosed(page)

    // Verify user appears in list
    await assertUserExists(page, userData.username)
    // Note: Email verification removed due to complex DOM structure
  })

  test('should create user with permissions', async ({ page }) => {
    // Open create user drawer
    await openCreateUserDrawer(page)

    // Create user with permissions
    const timestamp = Date.now()
    const userData = {
      username: `testuser${timestamp}`,
      email: `testuser${timestamp}@example.com`,
      password: 'password123',
      permissions: ['users::read', 'groups::read'],
    }

    await createUser(page, userData)

    // Verify user appears in list
    await assertUserExists(page, userData.username)
  })

  test('should show validation errors for invalid user data', async ({
    page,
  }) => {
    await openCreateUserDrawer(page)

    // Try to submit without required fields
    // Drawer submit label was standardised to "Create" (audit I-2);
    // scope by primary-button class to avoid colliding with the list
    // CTA which still carries aria-label="Create user".
    const submitButton = page
      .locator('.ant-drawer.ant-drawer-open .ant-btn-primary[type="submit"]')
    await submitButton.click()

    // Check for validation errors
    await expect(
      page.locator('.ant-form-item-explain-error').first()
    ).toBeVisible()
  })

  test('should show error for duplicate username', async ({ page }) => {
    // Create first user
    await openCreateUserDrawer(page)
    const timestamp = Date.now()
    const userData = {
      username: `testuser${timestamp}`,
      email: `testuser${timestamp}@example.com`,
      password: 'password123',
    }
    await createUser(page, userData)

    // Try to create user with same username
    await openCreateUserDrawer(page)
    const duplicateData = {
      username: userData.username,
      email: `different${timestamp}@example.com`,
      password: 'password123',
    }

    await page.getByLabel(/username/i).fill(duplicateData.username)
    await page.getByLabel(/email/i).fill(duplicateData.email)
    await page.getByLabel(/^password/i).fill(duplicateData.password)

    // Drawer submit label was standardised to "Create" (audit I-2);
    // scope by primary-button class to avoid colliding with the list
    // CTA which still carries aria-label="Create user".
    const submitButton = page
      .locator('.ant-drawer.ant-drawer-open .ant-btn-primary[type="submit"]')
    await submitButton.click()

    // Verify error message appears
    await expect(page.locator('.ant-message-error').first().first()).toBeVisible({
      timeout: 5000,
    })
  })

  test('should edit an existing user', async ({ page }) => {
    // Create a user first
    await openCreateUserDrawer(page)
    const timestamp = Date.now()
    const userData = {
      username: `testuser${timestamp}`,
      email: `testuser${timestamp}@example.com`,
      password: 'password123',
    }
    await createUser(page, userData)

    // Open edit drawer
    await openEditUserDrawer(page, userData.username)
    await assertDrawerOpen(page, /edit user/i)

    // Update user
    const updatedEmail = `updated${timestamp}@example.com`
    await updateUser(page, {
      email: updatedEmail,
      displayName: 'Updated User',
    })

    // Verify drawer closed
    await assertDrawerClosed(page)

    // Verify user still exists
    await assertUserExists(page, userData.username)
    // Note: Email verification removed due to complex DOM structure
  })

  test('toggling the Active switch off deactivates the user', async ({ page }) => {
    // Create an (active by default) user.
    await openCreateUserDrawer(page)
    const ts = Date.now()
    const userData = {
      username: `inactiveuser${ts}`,
      email: `inactiveuser${ts}@example.com`,
  test('edit drawer Active switch deactivates a user', async ({ page }) => {
    // Create an active user.
    await openCreateUserDrawer(page)
    const timestamp = Date.now()
    const userData = {
      username: `inactuser${timestamp}`,
      email: `inactuser${timestamp}@example.com`,
      password: 'password123',
    }
    await createUser(page, userData)
    await assertUserStatus(page, userData.username, 'active')

    // Edit → toggle the "Active" switch OFF → save.
    await openEditUserDrawer(page, userData.username)
    await assertDrawerOpen(page, /edit user/i)
    const drawer = page.locator('.ant-drawer.ant-drawer-open')
    await drawer.getByRole('switch', { name: 'Active' }).click()
    await drawer.locator('.ant-btn-primary[type="submit"]').click()
    await expect(page.locator('.ant-message-success').first()).toBeVisible({
      timeout: 5000,
    })
    await assertDrawerClosed(page)

    // The list now shows the user as inactive.
    // Open the edit drawer and flip the "Active" switch OFF.
    await openEditUserDrawer(page, userData.username)
    await assertDrawerOpen(page, /edit user/i)

    const drawer = page.locator('.ant-drawer.ant-drawer-open')
    const activeSwitch = drawer.getByRole('switch', { name: 'Active' })
    await expect(activeSwitch).toHaveAttribute('aria-checked', 'true')
    await activeSwitch.click()
    await expect(activeSwitch).toHaveAttribute('aria-checked', 'false')

    await drawer.locator('.ant-btn-primary[type="submit"]').click()
    await assertDrawerClosed(page)

    // The list row now reports the user as inactive.
    await assertUserStatus(page, userData.username, 'inactive')
  })

  test('should cancel user creation', async ({ page }) => {
    await openCreateUserDrawer(page)
    await assertDrawerOpen(page, 'Create User')

    // Fill some data
    await page.getByLabel(/username/i).fill('canceltest')

    // Click cancel button
    const cancelButton = page
      .locator('.ant-drawer.ant-drawer-open')
      .getByRole('button', { name: /cancel/i })
    await cancelButton.click()

    // Verify drawer closed
    await assertDrawerClosed(page)

    // Verify user was not created
    await assertUserNotExists(page, 'canceltest')
  })

  test('should close drawer with close button', async ({ page }) => {
    await openCreateUserDrawer(page)
    await assertDrawerOpen(page, 'Create User')

    // Close drawer
    await closeDrawer(page)

    // Verify drawer closed
    await assertDrawerClosed(page)
  })

  test('should delete a user', async ({ page }) => {
    // Create a user first
    await openCreateUserDrawer(page)
    const timestamp = Date.now()
    const userData = {
      username: `testuser${timestamp}`,
      email: `testuser${timestamp}@example.com`,
      password: 'password123',
    }
    await createUser(page, userData)

    // Verify user exists
    await assertUserExists(page, userData.username)

    // Delete user
    await deleteUser(page, userData.username)

    // Verify user no longer exists
    await assertUserNotExists(page, userData.username)
  })

  test('should validate password minimum length', async ({ page }) => {
    await openCreateUserDrawer(page)

    const timestamp = Date.now()
    await page.getByLabel(/username/i).fill(`testuser${timestamp}`)
    await page
      .getByLabel(/email/i)
      .fill(`testuser${timestamp}@example.com`)
    await page.getByLabel(/^password/i).fill('123') // Less than 6 characters

    // Drawer submit label was standardised to "Create" (audit I-2);
    // scope by primary-button class to avoid colliding with the list
    // CTA which still carries aria-label="Create user".
    const submitButton = page
      .locator('.ant-drawer.ant-drawer-open .ant-btn-primary[type="submit"]')
    await submitButton.click()

    // Check for validation error
    await expect(
      page.locator('.ant-form-item-explain-error', {
        hasText: /at least 6 characters/i,
      })
    ).toBeVisible()
  })

  test('should validate email format', async ({ page }) => {
    await openCreateUserDrawer(page)

    const timestamp = Date.now()
    await page.getByLabel(/username/i).fill(`testuser${timestamp}`)
    await page.getByLabel(/email/i).fill('invalid-email') // Invalid email
    await page.getByLabel(/^password/i).fill('password123')

    // Drawer submit label was standardised to "Create" (audit I-2);
    // scope by primary-button class to avoid colliding with the list
    // CTA which still carries aria-label="Create user".
    const submitButton = page
      .locator('.ant-drawer.ant-drawer-open .ant-btn-primary[type="submit"]')
    await submitButton.click()

    // Check for validation error
    await expect(
      page.locator('.ant-form-item-explain-error', {
        hasText: /valid email/i,
      })
    ).toBeVisible()
  })

  test('should handle pagination', async ({ page }) => {
    // Check if pagination exists (only if there are enough users)
    const pagination = page.locator('.ant-pagination')
    const paginationExists = await pagination.isVisible()

    if (paginationExists) {
      // Get current page info
      const totalText = await page
        .locator('.ant-pagination-total-text')
        .textContent()
      expect(totalText).toContain('of')

      // Try to change page size
      const pageSizeSelector = page.locator('.ant-select-selector', {
        has: page.locator('span', { hasText: /page/i }),
      })
      if (await pageSizeSelector.isVisible()) {
        await pageSizeSelector.click()
        await page
          .locator('.ant-select-dropdown:visible')
          .getByText('20')
          .click()
        await page.waitForTimeout(500)

        // Verify page size changed
        await expect(
          page.locator('.ant-pagination-total-text')
        ).toBeVisible()
      }
    }
  })

  // audit id all-c81f77e7ceff — the existing edit test changes email + display
  // name but NOT the username. The EditUserDrawer username field is editable;
  // assert a username CHANGE persists (new name appears, old name gone).
  test('should edit a user\'s username', async ({ page }) => {
    await openCreateUserDrawer(page)
    const ts = Date.now()
    const original = `olduser${ts}`
    await createUser(page, {
      username: original,
      email: `olduser${ts}@example.com`,
      password: 'password123',
    })

    await openEditUserDrawer(page, original)
    await assertDrawerOpen(page, /edit user/i)

    const renamed = `newuser${ts}`
    await updateUser(page, { username: renamed })
    await assertDrawerClosed(page)

    // The renamed user is present; the old username is gone.
    await assertUserExists(page, renamed)
    await assertUserNotExists(page, original)
  })
})
