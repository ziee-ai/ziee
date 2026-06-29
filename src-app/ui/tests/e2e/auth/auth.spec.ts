import { test, expect } from '../../fixtures/test-context'
import type { Page } from '@playwright/test'
import { assertNoAccessibilityViolations } from '../../utils/accessibility'
import { setTheme, isDarkMode } from '../../utils/theme'
import { byTestId } from '../testid'

// Create the first admin via the setup flow, then drop auth state and land
// back on the (logged-out) auth page so each test starts from a clean login form.
async function setupAdminThenAuthPage(page: Page, baseURL: string) {
  await page.goto(`${baseURL}/setup`)
  await byTestId(page, 'app-setup-username-input').waitFor({ timeout: 30000 })
  await byTestId(page, 'app-setup-username-input').fill('admin')
  await byTestId(page, 'app-setup-email-input').fill('admin@example.com')
  await byTestId(page, 'app-setup-password-input').fill('password123')
  await byTestId(page, 'app-setup-confirm-password-input').fill('password123')
  await byTestId(page, 'app-setup-submit-button').click()
  // First-time admin lands on the onboarding wizard or the home page.
  await expect(page).toHaveURL(/\/(onboarding|$)/, { timeout: 15000 })

  await page.evaluate(() => {
    localStorage.clear()
    sessionStorage.clear()
  })

  await page.goto(`${baseURL}/auth`, { waitUntil: 'load' })
  await byTestId(page, 'auth-login-username').waitFor({ timeout: 30000 })
}

async function gotoRegister(page: Page) {
  await byTestId(page, 'auth-login-switch-to-register').click()
  await expect(byTestId(page, 'auth-register-form')).toBeVisible()
}

test.describe('Authentication', () => {
  test('should pass accessibility checks', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await setupAdminThenAuthPage(page, baseURL)
    await assertNoAccessibilityViolations(page)
  })

  test('should pass accessibility checks in dark mode', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await setupAdminThenAuthPage(page, baseURL)

    await setTheme(page, 'dark')
    await byTestId(page, 'auth-login-username').waitFor({ timeout: 30000 })

    const darkModeActive = await isDarkMode(page)
    expect(darkModeActive).toBe(true)

    await assertNoAccessibilityViolations(page)
  })

  test('should display login form by default', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await setupAdminThenAuthPage(page, baseURL)

    // Should show login form fields + actions
    await expect(byTestId(page, 'auth-login-form')).toBeVisible()
    await expect(byTestId(page, 'auth-login-username')).toBeVisible()
    await expect(byTestId(page, 'auth-login-password')).toBeVisible()
    await expect(byTestId(page, 'auth-login-submit')).toBeVisible()

    // Should show switch to register link
    await expect(byTestId(page, 'auth-login-switch-to-register')).toBeVisible()
  })

  test('should validate required fields on login form', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await setupAdminThenAuthPage(page, baseURL)

    // Try to submit without filling form
    await byTestId(page, 'auth-login-submit').click()

    // Both required fields should surface a validation error
    await expect(
      byTestId(page, 'auth-login-form').getByRole('alert')
    ).toHaveCount(2)
  })

  test('should switch to register form', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await setupAdminThenAuthPage(page, baseURL)

    await gotoRegister(page)

    await expect(byTestId(page, 'auth-register-form')).toBeVisible()
    await expect(byTestId(page, 'auth-register-email')).toBeVisible()
    await expect(byTestId(page, 'auth-register-confirm-password')).toBeVisible()
    await expect(byTestId(page, 'auth-register-submit')).toBeVisible()
  })

  test('should display registration form fields', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await setupAdminThenAuthPage(page, baseURL)
    await gotoRegister(page)

    await expect(byTestId(page, 'auth-register-username')).toBeVisible()
    await expect(byTestId(page, 'auth-register-email')).toBeVisible()
    await expect(byTestId(page, 'auth-register-password')).toBeVisible()
    await expect(byTestId(page, 'auth-register-confirm-password')).toBeVisible()
  })

  test('should validate username minimum length on registration', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await setupAdminThenAuthPage(page, baseURL)
    await gotoRegister(page)

    await byTestId(page, 'auth-register-username').fill('ab')
    await byTestId(page, 'auth-register-email').fill('test@example.com')
    await byTestId(page, 'auth-register-password').fill('password123')

    // Trigger validation by blurring the username field
    await byTestId(page, 'auth-register-email').click()

    await expect(
      byTestId(page, 'auth-register-form').getByRole('alert')
    ).toHaveCount(1)
  })

  test('should validate email format on registration', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await setupAdminThenAuthPage(page, baseURL)
    await gotoRegister(page)

    await byTestId(page, 'auth-register-username').fill('testuser')
    await byTestId(page, 'auth-register-email').fill('not-an-email')
    await byTestId(page, 'auth-register-password').fill('password123')

    await byTestId(page, 'auth-register-password').click()

    await expect(
      byTestId(page, 'auth-register-form').getByRole('alert')
    ).toHaveCount(1)
  })

  test('should validate password minimum length on registration', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await setupAdminThenAuthPage(page, baseURL)
    await gotoRegister(page)

    await byTestId(page, 'auth-register-username').fill('testuser')
    await byTestId(page, 'auth-register-email').fill('test@example.com')
    await byTestId(page, 'auth-register-password').fill('pass')
    await byTestId(page, 'auth-register-confirm-password').fill('pass')

    await byTestId(page, 'auth-register-confirm-password').click()

    await expect(
      byTestId(page, 'auth-register-form').getByRole('alert')
    ).toHaveCount(1)
  })

  test('should validate password confirmation match', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await setupAdminThenAuthPage(page, baseURL)
    await gotoRegister(page)

    await byTestId(page, 'auth-register-username').fill('testuser')
    await byTestId(page, 'auth-register-email').fill('test@example.com')
    await byTestId(page, 'auth-register-password').fill('password123')
    await byTestId(page, 'auth-register-confirm-password').fill('password456')

    // Trigger validation by blurring the field
    await byTestId(page, 'auth-register-username').click()

    await expect(
      byTestId(page, 'auth-register-form').getByRole('alert')
    ).toHaveCount(1)
  })

  test('should switch back to login form', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await setupAdminThenAuthPage(page, baseURL)
    await gotoRegister(page)

    await byTestId(page, 'auth-register-switch-to-login').click()

    await expect(byTestId(page, 'auth-login-form')).toBeVisible()
    await expect(byTestId(page, 'auth-login-submit')).toBeVisible()
  })

  test('should register new user successfully', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await setupAdminThenAuthPage(page, baseURL)
    await gotoRegister(page)

    await byTestId(page, 'auth-register-username').fill('testuser')
    await byTestId(page, 'auth-register-email').fill('test@example.com')
    await byTestId(page, 'auth-register-password').fill('password123')
    await byTestId(page, 'auth-register-confirm-password').fill('password123')

    await byTestId(page, 'auth-register-submit').click()

    await expect(page).toHaveURL(/\/(onboarding|$)/, { timeout: 15000 })
  })

  test('should login with valid credentials', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await setupAdminThenAuthPage(page, baseURL)

    // Register a regular user
    await gotoRegister(page)
    await byTestId(page, 'auth-register-username').fill('testuser')
    await byTestId(page, 'auth-register-email').fill('test@example.com')
    await byTestId(page, 'auth-register-password').fill('password123')
    await byTestId(page, 'auth-register-confirm-password').fill('password123')
    await byTestId(page, 'auth-register-submit').click()
    await expect(page).toHaveURL(/\/(onboarding|$)/, { timeout: 15000 })

    await page.evaluate(() => {
      localStorage.clear()
      sessionStorage.clear()
    })

    await page.goto(`${baseURL}/auth`, { waitUntil: 'load' })
    await byTestId(page, 'auth-login-username').waitFor({ timeout: 30000 })

    await byTestId(page, 'auth-login-username').fill('testuser')
    await byTestId(page, 'auth-login-password').fill('password123')
    await byTestId(page, 'auth-login-submit').click()

    await expect(page).toHaveURL(/\/(onboarding|$)/, { timeout: 15000 })
  })

  test('should login with email instead of username', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await setupAdminThenAuthPage(page, baseURL)

    // Register a regular user
    await gotoRegister(page)
    await byTestId(page, 'auth-register-username').fill('testuser')
    await byTestId(page, 'auth-register-email').fill('test@example.com')
    await byTestId(page, 'auth-register-password').fill('password123')
    await byTestId(page, 'auth-register-confirm-password').fill('password123')
    await byTestId(page, 'auth-register-submit').click()
    await expect(page).toHaveURL(/\/(onboarding|$)/, { timeout: 15000 })

    await page.evaluate(() => {
      localStorage.clear()
      sessionStorage.clear()
    })

    await page.goto(`${baseURL}/auth`, { waitUntil: 'load' })
    await byTestId(page, 'auth-login-username').waitFor({ timeout: 30000 })

    // Login with email
    await byTestId(page, 'auth-login-username').fill('test@example.com')
    await byTestId(page, 'auth-login-password').fill('password123')
    await byTestId(page, 'auth-login-submit').click()

    await expect(page).toHaveURL(/\/(onboarding|$)/, { timeout: 15000 })
  })

  test('should validate all required fields on registration', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await setupAdminThenAuthPage(page, baseURL)
    await gotoRegister(page)

    // Try to submit without filling form
    await byTestId(page, 'auth-register-submit').click()

    // All four required fields should surface a validation error
    await expect(
      byTestId(page, 'auth-register-form').getByRole('alert')
    ).toHaveCount(4)
  })
})
