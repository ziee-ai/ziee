import { test, expect } from '../../fixtures/test-context'

/**
 * E2E — the SetupPage renders a server-error Alert when admin setup fails
 * (audit gap all-8eee55bb32b7).
 *
 * SetupPage.tsx renders `<Alert type="error" title={setupError}/>` only when
 * the store's `setupError` is set; App.store.ts `setupAdmin` populates it from
 * the thrown error's message in its `catch` (App.store.ts:71-81). The existing
 * setup.spec.ts covers the happy-path redirect and client-side field
 * validation, but never the server-error-Alert branch.
 *
 * The api-client (core.ts) parses a non-OK JSON body's `.error` field into the
 * thrown Error's message, which the store surfaces as `setupError`. So we make
 * the REAL POST /api/app/setup/admin return a 500 with `{"error": ...}` via
 * page.route — the server returning an error is the legitimate external
 * boundary here; the behavior under test is the UI's error-Alert rendering, not
 * the backend. The recovery leg then removes the interception and lets the real
 * backend serve the real success, proving the error state is transient and the
 * happy path still works.
 */

const FORCED_ERROR = 'Database unavailable during setup (E2E forced error)'

test.describe('App Setup — server-error alert', () => {
  test('a failed setup request renders the server-error Alert and keeps the user on /setup', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra

    // Force the setup POST to fail with a server error carrying a recognizable
    // message. core.ts maps a non-OK JSON body's `.error` into the thrown
    // Error's message, which the store stores as `setupError`.
    let failNext = true
    await page.route('**/api/app/setup/admin', async route => {
      if (failNext) {
        await route.fulfill({
          status: 500,
          contentType: 'application/json',
          body: JSON.stringify({ error: FORCED_ERROR }),
        })
      } else {
        await route.continue()
      }
    })

    await page.goto(`${baseURL}/setup`)
    await page.getByLabel('Username').waitFor({ timeout: 30000 })

    await page.getByLabel('Username').fill('admin')
    await page.getByLabel('Email').fill('admin@example.com')
    await page.getByLabel('Password', { exact: true }).fill('password123')
    await page.getByLabel('Confirm Password').fill('password123')

    await page.getByRole('button', { name: /create admin account/i }).click()

    // The server-error Alert (role=alert) appears carrying the server message,
    // and the user is NOT redirected — setup did not complete.
    const alert = page.getByRole('alert')
    await expect(alert).toBeVisible({ timeout: 10000 })
    await expect(alert).toContainText(FORCED_ERROR)
    await expect(page).toHaveURL(`${baseURL}/setup`)

    // Recovery: stop forcing the failure and resubmit (the form fields are
    // still populated). The real backend now serves the real success and the
    // app redirects home — proving the error state cleared on a real retry,
    // not a fake.
    failNext = false
    await page.getByRole('button', { name: /create admin account/i }).click()
    await expect(page).toHaveURL(`${baseURL}/`, { timeout: 15000 })
  })
})
