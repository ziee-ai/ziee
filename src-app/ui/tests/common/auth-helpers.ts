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

  // Navigate to setup page to check if admin exists
  await page.goto(`${baseURL}/setup`)
  await page.waitForLoadState('networkidle')

  // Wait for React to initialize and check if setup form appears or page redirects
  // The setup page will redirect to /auth if admin already exists
  try {
    // Wait for either the setup form to appear OR a redirect to happen
    await Promise.race([
      page.waitForSelector('#setup-form_username', { timeout: 5000 }),
      page.waitForURL(/\/auth/, { timeout: 5000 }),
      page.waitForURL(/\/$/, { timeout: 5000 }) // Sometimes redirects to home
    ])
  } catch {
    // If both timeout, wait a bit more and check URL
    await page.waitForTimeout(1000)
  }

  // Check if we're still on setup page (admin doesn't exist) or redirected (admin exists)
  const currentURL = page.url()
  const needsSetup = currentURL.includes('/setup')

  if (needsSetup) {
    // Admin doesn't exist - create it via setup form (form name: setup-form)
    await page.waitForSelector('#setup-form_username', { timeout: 30000 })
    await page.fill('#setup-form_username', username)
    await page.fill('#setup-form_email', email)
    await page.fill('#setup-form_password', password)
    await page.fill('#setup-form_confirm_password', password)
    await page.click('button[type="submit"]')

    // Wait for navigation to home
    await expect(page).toHaveURL(`${baseURL}/`, { timeout: 15000 })

    // CRITICAL: Wait for authentication token to be stored in localStorage
    await page.waitForFunction(
      () => {
        const authStorage = localStorage.getItem('auth-storage')
        if (!authStorage) return false
        try {
          const parsed = JSON.parse(authStorage)
          return parsed.state?.token !== null && parsed.state?.token !== undefined
        } catch {
          return false
        }
      },
      { timeout: 10000 }
    )
  } else {
    // Admin already exists - login instead
    await page.goto(`${baseURL}/auth`)
    await page.waitForSelector('#login_username', { timeout: 30000 })
    await page.fill('#login_username', username)
    await page.fill('#login_password', password)
    await page.click('button:has-text("Sign In")')

    // Wait for navigation to home
    await expect(page).toHaveURL(`${baseURL}/`, { timeout: 15000 })

    // Wait for token to be stored
    await page.waitForFunction(
      () => {
        const authStorage = localStorage.getItem('auth-storage')
        if (!authStorage) return false
        try {
          const parsed = JSON.parse(authStorage)
          return parsed.state?.token !== null && parsed.state?.token !== undefined
        } catch {
          return false
        }
      },
      { timeout: 10000 }
    )
  }
}

/**
 * Login as a specific user
 */
export async function login(
  page: Page,
  baseURL: string,
  username: string,
  password: string
) {
  await page.goto(`${baseURL}/auth`)
  await page.waitForSelector('#login_username', { timeout: 30000 })
  await page.fill('#login_username', username)
  await page.fill('#login_password', password)
  await page.click('button:has-text("Sign In")')

  // Wait for navigation to home
  await expect(page).toHaveURL(`${baseURL}/`, { timeout: 15000 })

  // Wait for token to be stored
  await page.waitForFunction(
    () => {
      const authStorage = localStorage.getItem('auth-storage')
      if (!authStorage) return false
      try {
        const parsed = JSON.parse(authStorage)
        return parsed.state?.token !== null && parsed.state?.token !== undefined
      } catch {
        return false
      }
    },
    { timeout: 10000 }
  )
}

/**
 * Create a test user via API
 */
export async function createTestUser(
  apiURL: string,
  adminToken: string,
  username: string,
  email: string,
  password: string,
  permissions: string[] = []
): Promise<string> {
  const response = await fetch(`${apiURL}/api/users`, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      Authorization: `Bearer ${adminToken}`,
    },
    body: JSON.stringify({
      username,
      email,
      password,
      permissions,
    }),
  })

  if (!response.ok) {
    const text = await response.text()
    throw new Error(`Failed to create user: ${response.statusText} - ${text}`)
  }

  const data = await response.json()
  return data.id
}

/**
 * Get admin token for API calls
 *
 * This makes a direct API call to get a fresh token for API operations.
 * Assumes admin user exists with default credentials.
 */
export async function getAdminToken(
  apiURL: string,
  credentials: AdminCredentials = DEFAULT_ADMIN_CREDENTIALS
): Promise<string> {
  const {
    username = 'admin',
    password = 'password123',
  } = credentials

  const response = await fetch(`${apiURL}/api/auth/login`, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({
      username,
      password,
    }),
  })

  if (!response.ok) {
    const text = await response.text()
    throw new Error(`Failed to get admin token: ${response.statusText} - ${text}`)
  }

  const data = await response.json()
  return data.access_token
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
