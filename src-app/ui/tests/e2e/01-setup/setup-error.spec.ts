import { test, expect } from '../../fixtures/test-context'

/**
 * E2E — SetupPage surfaces a server-side error.
 *
 * Covers SetupPage.tsx (`{setupError && <Alert type="error" ... />}`) +
 * App.store.ts setupAdmin() catch branch which lifts the upstream
 * message into `setupError`. Only the external HTTP boundary (the
 * setup-admin POST) is mocked; the store error path + the alert render
 * for real.
 */

const SETUP_ADMIN = '**/api/app/setup/admin'

test.describe('App Setup — server error', () => {
  test('shows the error alert when the setup POST fails', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra

    await page.route(SETUP_ADMIN, async route => {
      if (route.request().method() === 'POST') {
        await route.fulfill({
          status: 500,
          contentType: 'application/json',
          body: JSON.stringify({ message: 'Admin already exists' }),
        })
      } else {
        await route.fallback()
      }
    })

    await page.goto(`${baseURL}/setup`)
    await page.getByLabel('Username').waitFor({ timeout: 30000 })

    await page.getByLabel('Username').fill('admin')
    await page.getByLabel('Email').fill('admin@example.com')
    await page.getByLabel('Password', { exact: true }).fill('password123')
    await page.getByLabel('Confirm Password').fill('password123')
    await page.getByRole('button', { name: /create admin/i }).click()

    // The upstream message bubbles up into the page-level Alert.
    await expect(page.getByText('Admin already exists')).toBeVisible({
      timeout: 30000,
    })
  })
})
