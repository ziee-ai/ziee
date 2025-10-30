import { Page } from '@playwright/test'

export interface TestUser {
  username: string
  email: string
  password: string
  token?: string
  userId?: string
}

/**
 * Register a new user via API
 */
export async function registerUser(
  apiURL: (path: string) => string,
  user: TestUser
): Promise<{ token: string; userId: string }> {
  const response = await fetch(apiURL('/auth/register'), {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      username: user.username,
      email: user.email,
      password: user.password,
    }),
  })

  if (!response.ok) {
    const errorText = await response.text()
    throw new Error(`Failed to register user: ${response.statusText} - ${errorText}`)
  }

  const data = await response.json()
  return {
    token: data.access_token,
    userId: data.user.id,
  }
}

/**
 * Login via API and get token
 */
export async function loginUser(
  apiURL: (path: string) => string,
  username: string,
  password: string
): Promise<{ token: string; userId: string }> {
  const response = await fetch(apiURL('/auth/login'), {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      username_or_email: username,
      password,
    }),
  })

  if (!response.ok) {
    const errorText = await response.text()
    throw new Error(`Failed to login: ${response.statusText} - ${errorText}`)
  }

  const data = await response.json()
  return {
    token: data.access_token,
    userId: data.user.id,
  }
}

/**
 * Login via UI
 */
export async function loginViaUI(
  page: Page,
  baseURL: string,
  username: string,
  password: string
) {
  await page.goto(`${baseURL}/auth/login`)
  await page.fill('input[name="username_or_email"]', username)
  await page.fill('input[name="password"]', password)
  await page.click('button[type="submit"]')

  // Wait for redirect after successful login
  await page.waitForURL(`${baseURL}/dashboard`, { timeout: 10000 })
}

/**
 * Set auth token in browser storage
 */
export async function setAuthToken(page: Page, token: string) {
  await page.evaluate((t) => {
    localStorage.setItem('access_token', t)
  }, token)
}

/**
 * Create authenticated context
 */
export async function createAuthenticatedPage(
  page: Page,
  baseURL: string,
  token: string
) {
  // Set token
  await page.goto(baseURL)
  await setAuthToken(page, token)

  // Reload to apply auth
  await page.reload()
}
