import { Page, expect } from '@playwright/test'

/**
 * Auth-specific form helpers
 * These are only used within the auth test suite
 */

// =====================================================
// Admin Setup Helpers
// =====================================================

export async function createAdminViaSetup(
  page: Page,
  baseURL: string,
  username = 'admin',
  email = 'admin@example.com',
  password = 'password123'
) {
  await page.goto(`${baseURL}/setup`)
  await page.waitForSelector('#username', { timeout: 30000 })
  await page.fill('#username', username)
  await page.fill('#email', email)
  await page.fill('#password', password)
  await page.fill('#confirm_password', password)
  await page.click('button[type="submit"]')
  await expect(page).toHaveURL(`${baseURL}/`, { timeout: 15000 })
}

// =====================================================
// Login Form Helpers
// =====================================================

export async function fillLoginForm(
  page: Page,
  username: string,
  password: string
) {
  await page.fill('#login_username', username)
  await page.fill('#login_password', password)
}

export async function submitLoginForm(page: Page, baseURL: string) {
  await page.click('button:has-text("Sign In")')
  await expect(page).toHaveURL(`${baseURL}/`, { timeout: 15000 })
}

export async function loginWithCredentials(
  page: Page,
  baseURL: string,
  username: string,
  password: string
) {
  await fillLoginForm(page, username, password)
  await submitLoginForm(page, baseURL)
}

// =====================================================
// Registration Form Helpers
// =====================================================

export async function switchToRegistrationForm(page: Page) {
  await page.click('button:has-text("Sign Up")')
  await expect(page.locator('h3')).toContainText('Create Account')
}

export async function fillRegistrationForm(
  page: Page,
  username: string,
  email: string,
  password: string
) {
  await page.fill('#register_username', username)
  await page.fill('#register_email', email)
  await page.fill('#register_password', password)
  await page.fill('#register_confirmPassword', password)
}

export async function submitRegistrationForm(page: Page, baseURL: string) {
  await page.click('button:has-text("Sign Up")')
  await expect(page).toHaveURL(`${baseURL}/`, { timeout: 15000 })
}

export async function registerUser(
  page: Page,
  baseURL: string,
  username: string,
  email: string,
  password: string
) {
  await fillRegistrationForm(page, username, email, password)
  await submitRegistrationForm(page, baseURL)
}

export async function switchBackToLoginForm(page: Page) {
  await page.click('button:has-text("Sign In")')
  await expect(page.locator('label:has-text("Username or Email")')).toBeVisible()
}
