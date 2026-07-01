import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'

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
          // Match the real backend's AppError shape ({ error, error_code }),
          // which the API client parses via `.error`. A `{ message }` body is
          // not read, so the alert falls back to the bare "HTTP error!" string.
          body: JSON.stringify({ error: 'Admin already exists' }),
        })
      } else {
        await route.fallback()
      }
    })

    await page.goto(`${baseURL}/setup`)
    await byTestId(page, 'app-setup-username-input').waitFor({ timeout: 30000 })

    await byTestId(page, 'app-setup-username-input').fill('admin')
    await byTestId(page, 'app-setup-email-input').fill('admin@example.com')
    await byTestId(page, 'app-setup-password-input').fill('password123')
    await byTestId(page, 'app-setup-confirm-password-input').fill('password123')
    await byTestId(page, 'app-setup-submit-button').click()

    // The upstream message bubbles up into the page-level Alert.
    await expect(byTestId(page, 'app-setup-error-alert')).toContainText(
      'Admin already exists',
      { timeout: 30000 },
    )
  })
})
