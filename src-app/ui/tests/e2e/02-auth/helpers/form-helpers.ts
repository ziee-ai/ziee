import { Page, expect } from '@playwright/test'
import { byTestId } from '../../testid'

/**
 * Auth-specific form helpers
 * These are only used within the auth test suite
 *
 * Uses testid selectors (i18n-safe) — see tests/e2e/testid.ts
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
  await byTestId(page, 'app-setup-username-input').waitFor({ timeout: 30000 })
  await byTestId(page, 'app-setup-username-input').fill(username)
  await byTestId(page, 'app-setup-email-input').fill(email)
  await byTestId(page, 'app-setup-password-input').fill(password)
  await byTestId(page, 'app-setup-confirm-password-input').fill(password)
  await byTestId(page, 'app-setup-submit-button').click()
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
  await byTestId(page, 'auth-login-username').fill(username)
  await byTestId(page, 'auth-login-password').fill(password)
}

export async function submitLoginForm(page: Page, baseURL: string) {
  await byTestId(page, 'auth-login-submit').click()
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
  await byTestId(page, 'auth-login-switch-to-register').click()
  await expect(byTestId(page, 'auth-register-form')).toBeVisible()
}

export async function fillRegistrationForm(
  page: Page,
  username: string,
  email: string,
  password: string
) {
  await byTestId(page, 'auth-register-username').fill(username)
  await byTestId(page, 'auth-register-email').fill(email)
  await byTestId(page, 'auth-register-password').fill(password)
  await byTestId(page, 'auth-register-confirm-password').fill(password)
}

export async function submitRegistrationForm(page: Page, baseURL: string) {
  await byTestId(page, 'auth-register-submit').click()
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
  await byTestId(page, 'auth-register-switch-to-login').click()
  await expect(byTestId(page, 'auth-login-form')).toBeVisible()
}
