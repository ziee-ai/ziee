import { Page, expect } from '@playwright/test'

/**
 * Common authentication helpers used across multiple test suites
 * Only helpers that are used in 2+ different test suites should be here
 */

export interface AdminCredentials {
  username?: string
  email?: string
  password?: string
  displayName?: string
}

export const DEFAULT_ADMIN_CREDENTIALS: AdminCredentials = {
  username: 'admin',
  email: 'admin@example.com',
  password: 'password123',
  displayName: 'System Administrator',
}

/**
 * Login as admin user - creates admin if needed, otherwise logs in
 *
 * Used in: settings.spec.ts, hardware.spec.ts, llm.spec.ts
 */
export async function loginAsAdmin(
  page: Page,
  baseURL: string,
  credentials: AdminCredentials = DEFAULT_ADMIN_CREDENTIALS
) {
  const {
    username = 'admin',
    email = 'admin@example.com',
    password = 'password123',
  } = credentials

  // Try to go to setup page
  await page.goto(`${baseURL}/setup`)
  const usernameField = await page.locator('#username').count()

  if (usernameField > 0) {
    // Admin doesn't exist, create it
    await page.fill('#username', username)
    await page.fill('#email', email)
    await page.fill('#password', password)
    await page.fill('#confirm_password', password)
    await page.click('button[type="submit"]')
    await expect(page).toHaveURL(`${baseURL}/`, { timeout: 15000 })
  } else {
    // Admin already exists, need to login
    await page.goto(`${baseURL}/auth`)
    await page.waitForSelector('#login_username', { timeout: 30000 })
    await page.fill('#login_username', username)
    await page.fill('#login_password', password)
    await page.click('button:has-text("Sign In")')
    await expect(page).toHaveURL(`${baseURL}/`, { timeout: 15000 })
  }
}

/**
 * Clear authentication state (logout)
 *
 * Used in: auth.spec.ts and potentially others
 */
export async function clearAuthState(page: Page) {
  await page.evaluate(() => {
    localStorage.clear()
    sessionStorage.clear()
  })
}
