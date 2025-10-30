import { test, expect } from '../../fixtures/test-context'
import { assertNoAccessibilityViolations } from '../../utils/accessibility'
import { setTheme, isDarkMode } from '../../utils/theme'

test.describe('App Setup', () => {
  test('should pass accessibility checks', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await page.goto(`${baseURL}/setup`)

    // Wait for the form to be visible
    await page.waitForSelector('#username', { timeout: 30000 })

    // Check accessibility on the setup page
    await assertNoAccessibilityViolations(page)
  })

  test('should pass accessibility checks in dark mode', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await page.goto(`${baseURL}/setup`)

    // Wait for the form to be visible
    await page.waitForSelector('#username', { timeout: 30000 })

    // Switch to dark mode
    await setTheme(page, 'dark')
    await page.waitForSelector('#username', { timeout: 30000 })

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

    // Wait for the form to be visible
    await page.waitForSelector('#username', { timeout: 30000 })

    // Should show welcome message
    await expect(page.locator('h2')).toContainText('Welcome to Ziee Chat')
    await expect(page.locator('text=No administrator account exists')).toBeVisible()
  })

  test('should create admin account successfully', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await page.goto(`${baseURL}/setup`)

    // Wait for form to be visible
    await page.waitForSelector('#username', { timeout: 30000 })

    // Fill in the form
    await page.fill('#username', 'admin')
    await page.fill('#email', 'admin@example.com')
    await page.fill('#password', 'password123')
    await page.fill('#confirm_password', 'password123')
    await page.fill('#display_name', 'System Administrator')

    // Submit the form
    await page.click('button[type="submit"]')

    // Should redirect to home after successful setup
    await expect(page).toHaveURL(`${baseURL}/`, { timeout: 15000 })

  })

  test('should show validation error for short username', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await page.goto(`${baseURL}/setup`)

    // Wait for form to be visible
    await page.waitForSelector('#username', { timeout: 30000 })

    // Fill with short username
    await page.fill('#username', 'ab')
    await page.fill('#email', 'admin@example.com')
    await page.fill('#password', 'password123')
    await page.fill('#confirm_password', 'password123')

    // Trigger validation by clicking another field
    await page.click('#email')

    // Should show validation error
    await expect(page.locator('text=Username must be at least 3 characters')).toBeVisible()

  })

  test('should show validation error for invalid email', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await page.goto(`${baseURL}/setup`)

    // Wait for form to be visible
    await page.waitForSelector('#username', { timeout: 30000 })

    // Fill with invalid email
    await page.fill('#username','admin')
    await page.fill('#email','not-an-email')
    await page.fill('#password','password123')

    // Trigger validation
    await page.click('#password')

    // Should show validation error
    await expect(page.locator('text=Invalid email format')).toBeVisible()

  })

  test('should show validation error for short password', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await page.goto(`${baseURL}/setup`)

    // Wait for form to be visible
    await page.waitForSelector('#username', { timeout: 30000 })

    // Fill with short password
    await page.fill('#username','admin')
    await page.fill('#email','admin@example.com')
    await page.fill('#password','pass123')
    await page.fill('#confirm_password','pass123')

    // Try to submit the form
    await page.click('button[type="submit"]')

    // Should still be on setup page (submission failed due to validation)
    await expect(page).toHaveURL(`${baseURL}/setup`)

    // Password help text should be visible
    await expect(page.locator('text=Must be at least 8 characters')).toBeVisible()

  })

  test('should show validation error for mismatched passwords', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await page.goto(`${baseURL}/setup`)

    // Wait for form to be visible
    await page.waitForSelector('#username', { timeout: 30000 })

    // Fill with mismatched passwords
    await page.fill('#username','admin')
    await page.fill('#email','admin@example.com')
    await page.fill('#password','password123')
    await page.fill('#confirm_password','password456')

    // Trigger validation
    await page.click('#display_name')

    // Should show validation error
    await expect(page.locator('text=Passwords do not match')).toBeVisible()

  })

  test('should show validation error for invalid username characters', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await page.goto(`${baseURL}/setup`)

    // Wait for form to be visible
    await page.waitForSelector('#username', { timeout: 30000 })

    // Fill with invalid username (contains special characters)
    await page.fill('#username','admin@user')
    await page.fill('#email','admin@example.com')
    await page.fill('#password','password123')

    // Trigger validation
    await page.click('#email')

    // Should show validation error
    await expect(page.locator('text=Username can only contain letters, numbers, hyphens, and underscores')).toBeVisible()

  })

  test('should work with keyboard navigation only', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await page.goto(`${baseURL}/setup`)

    // Wait for form to be visible
    await page.waitForSelector('#username', { timeout: 30000 })

    // Tab to username field
    await page.keyboard.press('Tab')
    await page.keyboard.type('admin')

    // Tab to email field
    await page.keyboard.press('Tab')
    await page.keyboard.type('admin@example.com')

    // Tab to password field
    await page.keyboard.press('Tab')
    await page.keyboard.type('password123')

    // Tab to confirm password field
    await page.keyboard.press('Tab')
    await page.keyboard.type('password123')

    // Tab to display name field
    await page.keyboard.press('Tab')
    await page.keyboard.type('System Administrator')

    // Tab to submit button and press Enter
    await page.keyboard.press('Tab')
    await page.keyboard.press('Enter')

    // Should redirect to home
    await expect(page).toHaveURL(`${baseURL}/`, { timeout: 15000 })

  })

  test('should work without optional display name', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await page.goto(`${baseURL}/setup`)

    // Wait for form to be visible
    await page.waitForSelector('#username', { timeout: 30000 })

    // Fill form without display name
    await page.fill('#username','admin')
    await page.fill('#email','admin@example.com')
    await page.fill('#password','password123')
    await page.fill('#confirm_password','password123')
    // Skip display_name

    // Submit
    await page.click('button[type="submit"]')

    // Should redirect to home
    await expect(page).toHaveURL(`${baseURL}/`, { timeout: 15000 })

  })

  test('should show all required fields', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await page.goto(`${baseURL}/setup`)

    // Wait for form to be visible
    await page.waitForSelector('#username', { timeout: 30000 })

    // Check all fields are present
    await expect(page.locator('#username')).toBeVisible()
    await expect(page.locator('#email')).toBeVisible()
    await expect(page.locator('#password')).toBeVisible()
    await expect(page.locator('#confirm_password')).toBeVisible()
    await expect(page.locator('#display_name')).toBeVisible()
    await expect(page.getByRole('button', { name: /create admin account/i })).toBeVisible()

    // Check labels (using exact text to avoid conflicts)
    await expect(page.locator('label:has-text("Username")')).toBeVisible()
    await expect(page.locator('label:has-text("Email")')).toBeVisible()
    await expect(page.locator('label[for="password"]:has-text("Password")')).toBeVisible()
    await expect(page.locator('label:has-text("Confirm Password")')).toBeVisible()
    await expect(page.locator('label:has-text("Display Name (Optional)")')).toBeVisible()

  })

  test('should show password requirements', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await page.goto(`${baseURL}/setup`)

    // Wait for form to be visible
    await page.waitForSelector('#username', { timeout: 30000 })

    // Check that password help text is visible
    await expect(page.locator('text=Must be at least 8 characters')).toBeVisible()

  })

  test('should not allow empty form submission', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await page.goto(`${baseURL}/setup`)

    // Wait for form to be visible
    await page.waitForSelector('#username', { timeout: 30000 })

    // Try to submit without filling form
    await page.click('button[type="submit"]')

    // Should still be on setup page
    await expect(page).toHaveURL(`${baseURL}/setup`)

    // Should show validation errors
    await expect(page.locator('text=Username is required')).toBeVisible()

  })

  test('should handle duplicate username gracefully', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    // First, create an admin user
    await page.goto(`${baseURL}/setup`)

    // Wait for form to be visible
    await page.waitForSelector('#username', { timeout: 30000 })
    await page.fill('#username','admin')
    await page.fill('#email','admin@example.com')
    await page.fill('#password','password123')
    await page.fill('#confirm_password','password123')
    await page.click('button[type="submit"]')

    // Wait for setup to complete - should redirect to home page
    await expect(page).toHaveURL(`${baseURL}/`, { timeout: 15000 })

    // Try to access setup page again (should redirect since admin exists)
    await page.goto(`${baseURL}/setup`)

    // Should be redirected away from setup (either to login or home)
    await expect(page).not.toHaveURL(`${baseURL}/setup`)

  })
})
