import { test, expect } from '../../fixtures/test-context'
import { assertNoAccessibilityViolations } from '../../utils/accessibility'
import { setTheme, isDarkMode } from '../../utils/theme'

test.describe('App Setup', () => {
  test('should pass accessibility checks', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await page.goto(`${baseURL}/setup`)

    // Wait for the form to be visible using semantic selector
    await page.getByLabel('Username').waitFor({ timeout: 30000 })

    // Check accessibility on the setup page
    await assertNoAccessibilityViolations(page)
  })

  test('should pass accessibility checks in dark mode', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await page.goto(`${baseURL}/setup`)

    // Wait for the form to be visible using semantic selector
    await page.getByLabel('Username').waitFor({ timeout: 30000 })

    // Switch to dark mode
    await setTheme(page, 'dark')
    await page.getByLabel('Username').waitFor({ timeout: 30000 })

    // Verify dark mode is active
    const darkModeActive = await isDarkMode(page)
    expect(darkModeActive).toBe(true)

    // Check accessibility in dark mode
    await assertNoAccessibilityViolations(page)
  })

  test('should display setup page when no admin exists', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    // Navigate directly to setup page
    await page.goto(`${baseURL}`)

    // Wait for the form to be visible using semantic selector
    await page.getByLabel('Username').waitFor({ timeout: 30000 })

    // Should show welcome message
    await expect(page.getByRole('heading', { level: 2, name: /welcome to ziee chat/i })).toBeVisible()
    await expect(page.getByText('No administrator account exists')).toBeVisible()
  })

  test('should create admin account successfully', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await page.goto(`${baseURL}/setup`)

    // Wait for form to be visible using semantic selector
    await page.getByLabel('Username').waitFor({ timeout: 30000 })

    // Fill in the form using semantic selectors
    await page.getByLabel('Username').fill('admin')
    await page.getByLabel('Email').fill('admin@example.com')
    await page.getByLabel('Password', { exact: true }).fill('password123')
    await page.getByLabel('Confirm Password').fill('password123')
    await page.getByLabel(/display name.*optional/i).fill('System Administrator')

    // Submit the form using role-based selector
    await page.getByRole('button', { name: /create admin account/i }).click()

    // Should redirect to home after successful setup
    await expect(page).toHaveURL(`${baseURL}/`, { timeout: 15000 })

  })

  test('should show validation error for short username', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await page.goto(`${baseURL}/setup`)

    // Wait for form to be visible using semantic selector
    await page.getByLabel('Username').waitFor({ timeout: 30000 })

    // Fill with short username
    await page.getByLabel('Username').fill('ab')
    await page.getByLabel('Email').fill('admin@example.com')
    await page.getByLabel('Password', { exact: true }).fill('password123')
    await page.getByLabel('Confirm Password').fill('password123')

    // Trigger validation by clicking another field
    await page.getByLabel('Email').click()

    // Should show validation error
    await expect(page.getByText('Username must be at least 3 characters')).toBeVisible()

  })

  test('should show validation error for invalid email', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await page.goto(`${baseURL}/setup`)

    // Wait for form to be visible using semantic selector
    await page.getByLabel('Username').waitFor({ timeout: 30000 })

    // Fill with invalid email
    await page.getByLabel('Username').fill('admin')
    await page.getByLabel('Email').fill('not-an-email')
    await page.getByLabel('Password', { exact: true }).fill('password123')

    // Trigger validation
    await page.getByLabel('Password', { exact: true }).click()

    // Should show validation error
    await expect(page.getByText('Invalid email format')).toBeVisible()

  })

  test('should show validation error for short password', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await page.goto(`${baseURL}/setup`)

    // Wait for form to be visible using semantic selector
    await page.getByLabel('Username').waitFor({ timeout: 30000 })

    // Fill with short password
    await page.getByLabel('Username').fill('admin')
    await page.getByLabel('Email').fill('admin@example.com')
    await page.getByLabel('Password', { exact: true }).fill('pass123')
    await page.getByLabel('Confirm Password').fill('pass123')

    // Try to submit the form
    await page.getByRole('button', { name: /create admin account/i }).click()

    // Should still be on setup page (submission failed due to validation)
    await expect(page).toHaveURL(`${baseURL}/setup`)

    // Password help text should be visible
    await expect(page.getByText('Must be at least 8 characters')).toBeVisible()

  })

  test('should show validation error for mismatched passwords', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await page.goto(`${baseURL}/setup`)

    // Wait for form to be visible using semantic selector
    await page.getByLabel('Username').waitFor({ timeout: 30000 })

    // Fill with mismatched passwords
    await page.getByLabel('Username').fill('admin')
    await page.getByLabel('Email').fill('admin@example.com')
    await page.getByLabel('Password', { exact: true }).fill('password123')
    await page.getByLabel('Confirm Password').fill('password456')

    // Trigger validation
    await page.getByLabel(/display name.*optional/i).click()

    // Should show validation error
    await expect(page.getByText('Passwords do not match')).toBeVisible()

  })

  test('should show validation error for invalid username characters', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await page.goto(`${baseURL}/setup`)

    // Wait for form to be visible using semantic selector
    await page.getByLabel('Username').waitFor({ timeout: 30000 })

    // Fill with invalid username (contains special characters)
    await page.getByLabel('Username').fill('admin@user')
    await page.getByLabel('Email').fill('admin@example.com')
    await page.getByLabel('Password', { exact: true }).fill('password123')

    // Trigger validation
    await page.getByLabel('Email').click()

    // Should show validation error
    await expect(page.getByText('Username can only contain letters, numbers, hyphens, and underscores')).toBeVisible()

  })

  test('should work with keyboard navigation only', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await page.goto(`${baseURL}/setup`)

    // Wait for form to be visible using semantic selector
    await page.getByLabel('Username').waitFor({ timeout: 30000 })

    // NOTE on Tab counts: each antd `Input.Password` renders a focusable
    // eye-toggle (tabindex="0") AFTER its input, so reaching the next
    // field requires an extra Tab to step past the toggle. Without that,
    // the sequence shifts by one (the confirm field gets the display-name
    // text → "Passwords do not match" → the form never submits).

    // Tab to username field
    await page.keyboard.press('Tab')
    await page.keyboard.type('admin')

    // Tab to email field
    await page.keyboard.press('Tab')
    await page.keyboard.type('admin@example.com')

    // Tab to password field
    await page.keyboard.press('Tab')
    await page.keyboard.type('password123')

    // Tab past the password eye-toggle, then to the confirm-password field
    await page.keyboard.press('Tab')
    await page.keyboard.press('Tab')
    await page.keyboard.type('password123')

    // Tab past the confirm-password eye-toggle, then to the display-name field
    await page.keyboard.press('Tab')
    await page.keyboard.press('Tab')
    await page.keyboard.type('System Administrator')

    // Submit via Enter from the (text) display-name field — standard HTML
    // form submission (a final "Tab to the submit button" is fragile
    // because of the eye-toggles above).
    await page.keyboard.press('Enter')

    // Should redirect to home
    await expect(page).toHaveURL(`${baseURL}/`, { timeout: 15000 })

  })

  test('should work without optional display name', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await page.goto(`${baseURL}/setup`)

    // Wait for form to be visible using semantic selector
    await page.getByLabel('Username').waitFor({ timeout: 30000 })

    // Fill form without display name
    await page.getByLabel('Username').fill('admin')
    await page.getByLabel('Email').fill('admin@example.com')
    await page.getByLabel('Password', { exact: true }).fill('password123')
    await page.getByLabel('Confirm Password').fill('password123')
    // Skip display_name

    // Submit
    await page.getByRole('button', { name: /create admin account/i }).click()

    // Should redirect to home
    await expect(page).toHaveURL(`${baseURL}/`, { timeout: 15000 })

  })

  test('should show all required fields', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await page.goto(`${baseURL}/setup`)

    // Wait for form to be visible using semantic selector
    await page.getByLabel('Username').waitFor({ timeout: 30000 })

    // Check all fields are present using semantic selectors
    await expect(page.getByLabel('Username')).toBeVisible()
    await expect(page.getByLabel('Email')).toBeVisible()
    await expect(page.getByLabel('Password', { exact: true })).toBeVisible()
    await expect(page.getByLabel('Confirm Password')).toBeVisible()
    await expect(page.getByLabel(/display name.*optional/i)).toBeVisible()
    await expect(page.getByRole('button', { name: /create admin account/i })).toBeVisible()

    // Check labels are visible
    await expect(page.getByText('Username', { exact: false })).toBeVisible()
    await expect(page.getByText('Email', { exact: false })).toBeVisible()
    await expect(page.getByText('Password', { exact: false }).first()).toBeVisible()
    await expect(page.getByText('Confirm Password')).toBeVisible()
    await expect(page.getByText(/display name.*optional/i)).toBeVisible()

  })

  test('should show password requirements', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await page.goto(`${baseURL}/setup`)

    // Wait for form to be visible using semantic selector
    await page.getByLabel('Username').waitFor({ timeout: 30000 })

    // Check that password help text is visible
    await expect(page.getByText('Must be at least 8 characters')).toBeVisible()

  })

  test('should not allow empty form submission', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await page.goto(`${baseURL}/setup`)

    // Wait for form to be visible using semantic selector
    await page.getByLabel('Username').waitFor({ timeout: 30000 })

    // Try to submit without filling form
    await page.getByRole('button', { name: /create admin account/i }).click()

    // Should still be on setup page
    await expect(page).toHaveURL(`${baseURL}/setup`)

    // Should show validation errors
    await expect(page.getByText('Username is required')).toBeVisible()

  })

  test('should handle duplicate username gracefully', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    // First, create an admin user
    await page.goto(`${baseURL}/setup`)

    // Wait for form to be visible using semantic selector
    await page.getByLabel('Username').waitFor({ timeout: 30000 })
    await page.getByLabel('Username').fill('admin')
    await page.getByLabel('Email').fill('admin@example.com')
    await page.getByLabel('Password', { exact: true }).fill('password123')
    await page.getByLabel('Confirm Password').fill('password123')
    await page.getByRole('button', { name: /create admin account/i }).click()

    // Wait for setup to complete - should redirect to home page
    await expect(page).toHaveURL(`${baseURL}/`, { timeout: 15000 })

    // Try to access setup page again (should redirect since admin exists)
    await page.goto(`${baseURL}/setup`)

    // Should be redirected away from setup (either to login or home)
    await expect(page).not.toHaveURL(`${baseURL}/setup`)

  })
})
