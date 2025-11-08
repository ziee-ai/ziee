import { Page, expect } from '@playwright/test'

/**
 * Auth-specific form helpers
 * These are only used within the auth test suite
 *
 * Uses semantic selectors following CLAUDE.md best practices
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
  await page.getByLabel('Username').waitFor({ timeout: 30000 })
  await page.getByLabel('Username').fill(username)
  await page.getByLabel('Email').fill(email)
  await page.getByLabel('Password', { exact: true }).fill(password)
  await page.getByLabel('Confirm Password').fill(password)
  await page.getByRole('button', { name: /create admin account/i }).click()
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
  await page.getByLabel('Username or Email').fill(username)
  await page.getByLabel('Password', { exact: true }).fill(password)
}

export async function submitLoginForm(page: Page, baseURL: string) {
  await page.getByRole('button', { name: /^sign in$/i }).click()
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
  await page.getByRole('button', { name: /sign up/i }).click()
  await expect(page.getByRole('heading', { level: 3, name: /create account/i })).toBeVisible()
}

export async function fillRegistrationForm(
  page: Page,
  username: string,
  email: string,
  password: string
) {
  await page.getByLabel('Username').fill(username)
  await page.getByLabel('Email').fill(email)
  await page.getByLabel('Password', { exact: true }).fill(password)
  await page.getByLabel('Confirm Password').fill(password)
}

export async function submitRegistrationForm(page: Page, baseURL: string) {
  await page.getByRole('button', { name: /^sign up$/i }).click()
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
  await page.getByRole('button', { name: /^sign in$/i }).click()
  await expect(page.getByText('Username or Email')).toBeVisible()
}
