import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'
import { assertNoAccessibilityViolations } from '../../utils/accessibility'
import { setTheme, isDarkMode } from '../../utils/theme'

test.describe('App Setup', () => {
  test('should pass accessibility checks', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await page.goto(`${baseURL}/setup`)

    // Wait for the form to be visible
    await byTestId(page, 'app-setup-username-input').waitFor({ timeout: 30000 })

    // Check accessibility on the setup page
    await assertNoAccessibilityViolations(page)
  })

  test('should pass accessibility checks in dark mode', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await page.goto(`${baseURL}/setup`)

    // Wait for the form to be visible
    await byTestId(page, 'app-setup-username-input').waitFor({ timeout: 30000 })

    // Switch to dark mode
    await setTheme(page, 'dark')
    await byTestId(page, 'app-setup-username-input').waitFor({ timeout: 30000 })

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
    await byTestId(page, 'app-setup-username-input').waitFor({ timeout: 30000 })

    // Should show the welcome / no-admin message block + the setup card
    await expect(byTestId(page, 'app-setup-card')).toBeVisible()
    await expect(byTestId(page, 'app-setup-welcome')).toBeVisible()
  })

  test('should create admin account successfully', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await page.goto(`${baseURL}/setup`)

    // Wait for form to be visible
    await byTestId(page, 'app-setup-username-input').waitFor({ timeout: 30000 })

    // Fill in the form
    await byTestId(page, 'app-setup-username-input').fill('admin')
    await byTestId(page, 'app-setup-email-input').fill('admin@example.com')
    await byTestId(page, 'app-setup-password-input').fill('password123')
    await byTestId(page, 'app-setup-confirm-password-input').fill('password123')
    await byTestId(page, 'app-setup-display-name-input').fill('System Administrator')

    // Submit the form
    await byTestId(page, 'app-setup-submit-button').click()

    // Should redirect to home after successful setup
    await expect(page).toHaveURL(`${baseURL}/`, { timeout: 15000 })

  })

  test('should show validation error for short username', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await page.goto(`${baseURL}/setup`)

    // Wait for form to be visible
    await byTestId(page, 'app-setup-username-input').waitFor({ timeout: 30000 })

    // Fill with short username
    await byTestId(page, 'app-setup-username-input').fill('ab')
    await byTestId(page, 'app-setup-email-input').fill('admin@example.com')
    await byTestId(page, 'app-setup-password-input').fill('password123')
    await byTestId(page, 'app-setup-confirm-password-input').fill('password123')

    // Trigger validation by clicking another field
    await byTestId(page, 'app-setup-email-input').click()

    // Should show the username field's validation error
    await expect(byTestId(page, 'field-error-username')).toBeVisible()
    await expect(byTestId(page, 'field-error-username')).toContainText(
      'Username must be at least 3 characters',
    )

  })

  test('should show validation error for invalid email', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await page.goto(`${baseURL}/setup`)

    // Wait for form to be visible
    await byTestId(page, 'app-setup-username-input').waitFor({ timeout: 30000 })

    // Fill with invalid email
    await byTestId(page, 'app-setup-username-input').fill('admin')
    await byTestId(page, 'app-setup-email-input').fill('not-an-email')
    await byTestId(page, 'app-setup-password-input').fill('password123')

    // Trigger validation
    await byTestId(page, 'app-setup-password-input').click()

    // Should show the email field's validation error
    await expect(byTestId(page, 'field-error-email')).toBeVisible()
    await expect(byTestId(page, 'field-error-email')).toContainText('Invalid email format')

  })

  test('should accept a valid email with a 3+char TLD', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await page.goto(`${baseURL}/setup`)

    // Wait for form to be visible
    await byTestId(page, 'app-setup-username-input').waitFor({ timeout: 30000 })

    // A valid `.com` address (the reported regression) must NOT be flagged
    await byTestId(page, 'app-setup-username-input').fill('admin')
    await byTestId(page, 'app-setup-email-input').fill('khoi@gmail.com')
    await byTestId(page, 'app-setup-password-input').fill('password123')

    // Trigger validation by blurring the email field
    await byTestId(page, 'app-setup-password-input').click()

    // No email validation error should appear
    await expect(byTestId(page, 'field-error-email')).toHaveCount(0)

  })

  test('should show validation error for short password', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await page.goto(`${baseURL}/setup`)

    // Wait for form to be visible
    await byTestId(page, 'app-setup-username-input').waitFor({ timeout: 30000 })

    // Fill with short password
    await byTestId(page, 'app-setup-username-input').fill('admin')
    await byTestId(page, 'app-setup-email-input').fill('admin@example.com')
    await byTestId(page, 'app-setup-password-input').fill('pass123')
    await byTestId(page, 'app-setup-confirm-password-input').fill('pass123')

    // Try to submit the form
    await byTestId(page, 'app-setup-submit-button').click()

    // Should still be on setup page (submission failed due to validation)
    await expect(page).toHaveURL(`${baseURL}/setup`)

    // Password help text should be visible
    await expect(byTestId(page, 'field-desc-password')).toContainText(
      'Must be at least 8 characters',
    )

  })

  test('should show validation error for mismatched passwords', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await page.goto(`${baseURL}/setup`)

    // Wait for form to be visible
    await byTestId(page, 'app-setup-username-input').waitFor({ timeout: 30000 })

    // Fill with mismatched passwords
    await byTestId(page, 'app-setup-username-input').fill('admin')
    await byTestId(page, 'app-setup-email-input').fill('admin@example.com')
    await byTestId(page, 'app-setup-password-input').fill('password123')
    await byTestId(page, 'app-setup-confirm-password-input').fill('password456')

    // Trigger validation
    await byTestId(page, 'app-setup-display-name-input').click()

    // Should show the confirm-password field's validation error
    await expect(byTestId(page, 'field-error-confirm_password')).toBeVisible()
    await expect(byTestId(page, 'field-error-confirm_password')).toContainText(
      'Passwords do not match',
    )

  })

  test('should show validation error for invalid username characters', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await page.goto(`${baseURL}/setup`)

    // Wait for form to be visible
    await byTestId(page, 'app-setup-username-input').waitFor({ timeout: 30000 })

    // Fill with invalid username (contains special characters)
    await byTestId(page, 'app-setup-username-input').fill('admin@user')
    await byTestId(page, 'app-setup-email-input').fill('admin@example.com')
    await byTestId(page, 'app-setup-password-input').fill('password123')

    // Trigger validation
    await byTestId(page, 'app-setup-email-input').click()

    // Should show the username field's validation error
    await expect(byTestId(page, 'field-error-username')).toBeVisible()
    await expect(byTestId(page, 'field-error-username')).toContainText(
      'Username can only contain letters, numbers, hyphens, and underscores',
    )

  })

  test('should work with keyboard navigation only', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await page.goto(`${baseURL}/setup`)

    // Wait for form to be visible
    await byTestId(page, 'app-setup-username-input').waitFor({ timeout: 30000 })

    // NOTE on Tab counts: the setup form uses plain `<Input type="password">`
    // (NOT the kit PasswordInput), so there is NO focusable eye-toggle between
    // fields — a single Tab reaches the next input. (An extra Tab per password
    // field would shift the sequence by one: the value meant for Confirm lands
    // in Display Name, Confirm stays empty, and the form never submits.)

    // Focus the username field explicitly. The migrated Input renders with
    // autoFocus, so a leading Tab would step OFF username and shift the whole
    // sequence by one. Focusing directly makes the subsequent Tab counts
    // deterministic regardless of autoFocus timing.
    await byTestId(page, 'app-setup-username-input').focus()
    await page.keyboard.type('admin')

    // Tab to email field
    await page.keyboard.press('Tab')
    await page.keyboard.type('admin@example.com')

    // Tab to password field
    await page.keyboard.press('Tab')
    await page.keyboard.type('password123')

    // Tab to the confirm-password field
    await page.keyboard.press('Tab')
    await page.keyboard.type('password123')

    // Tab to the display-name field
    await page.keyboard.press('Tab')
    await page.keyboard.type('System Administrator')

    // Submit via Enter from the (text) display-name field — standard HTML
    // form submission (a final "Tab to the submit button" is fragile).
    await page.keyboard.press('Enter')

    // Should redirect to home
    await expect(page).toHaveURL(`${baseURL}/`, { timeout: 15000 })

  })

  test('should work without optional display name', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await page.goto(`${baseURL}/setup`)

    // Wait for form to be visible
    await byTestId(page, 'app-setup-username-input').waitFor({ timeout: 30000 })

    // Fill form without display name
    await byTestId(page, 'app-setup-username-input').fill('admin')
    await byTestId(page, 'app-setup-email-input').fill('admin@example.com')
    await byTestId(page, 'app-setup-password-input').fill('password123')
    await byTestId(page, 'app-setup-confirm-password-input').fill('password123')
    // Skip display_name

    // Submit
    await byTestId(page, 'app-setup-submit-button').click()

    // Should redirect to home
    await expect(page).toHaveURL(`${baseURL}/`, { timeout: 15000 })

  })

  test('should show all required fields', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await page.goto(`${baseURL}/setup`)

    // Wait for form to be visible
    await byTestId(page, 'app-setup-username-input').waitFor({ timeout: 30000 })

    // Check all fields are present
    await expect(byTestId(page, 'app-setup-form')).toBeVisible()
    await expect(byTestId(page, 'app-setup-username-input')).toBeVisible()
    await expect(byTestId(page, 'app-setup-email-input')).toBeVisible()
    await expect(byTestId(page, 'app-setup-password-input')).toBeVisible()
    await expect(byTestId(page, 'app-setup-confirm-password-input')).toBeVisible()
    await expect(byTestId(page, 'app-setup-display-name-input')).toBeVisible()
    await expect(byTestId(page, 'app-setup-submit-button')).toBeVisible()

  })

  test('should show password requirements', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await page.goto(`${baseURL}/setup`)

    // Wait for form to be visible
    await byTestId(page, 'app-setup-username-input').waitFor({ timeout: 30000 })

    // Check that password help text is visible
    await expect(byTestId(page, 'field-desc-password')).toContainText(
      'Must be at least 8 characters',
    )

  })

  test('should not allow empty form submission', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await page.goto(`${baseURL}/setup`)

    // Wait for form to be visible
    await byTestId(page, 'app-setup-username-input').waitFor({ timeout: 30000 })

    // Try to submit without filling form
    await byTestId(page, 'app-setup-submit-button').click()

    // Should still be on setup page
    await expect(page).toHaveURL(`${baseURL}/setup`)

    // Should show the username required validation error
    await expect(byTestId(page, 'field-error-username')).toBeVisible()
    await expect(byTestId(page, 'field-error-username')).toContainText('Username is required')

  })

  test('should handle duplicate username gracefully', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    // First, create an admin user
    await page.goto(`${baseURL}/setup`)

    // Wait for form to be visible
    await byTestId(page, 'app-setup-username-input').waitFor({ timeout: 30000 })
    await byTestId(page, 'app-setup-username-input').fill('admin')
    await byTestId(page, 'app-setup-email-input').fill('admin@example.com')
    await byTestId(page, 'app-setup-password-input').fill('password123')
    await byTestId(page, 'app-setup-confirm-password-input').fill('password123')
    await byTestId(page, 'app-setup-submit-button').click()

    // Wait for setup to complete - should redirect to home page
    await expect(page).toHaveURL(`${baseURL}/`, { timeout: 15000 })

    // Try to access setup page again (should redirect since admin exists)
    await page.goto(`${baseURL}/setup`)

    // Should be redirected away from setup (either to login or home)
    await expect(page).not.toHaveURL(`${baseURL}/setup`)

  })
})

