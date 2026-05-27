import { test, expect } from '@playwright/test'
import { installTauriMock, mockBackendDefaults } from './helpers/tauri-mock'

test.describe('desktop setup-redirect bypass', () => {
  test('never redirects to /setup even when the status API claims setup is needed', async ({
    page,
  }) => {
    await installTauriMock(page, { autoLogin: 'success' })
    await mockBackendDefaults(page)

    // Override the default setup-status mock to claim setup IS needed.
    // The desktop AuthGuard override must ignore this — bootstrap.rs
    // already created the admin server-side, and the user can never do
    // anything useful on the setup page in single-admin mode.
    await page.route('**/api/app/setup/status', async route => {
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({ needs_setup: true }),
      })
    })

    await page.goto('/')

    // Watch for ~3 s; if the desktop AuthGuard accidentally honoured
    // the status, the URL would have flipped to /setup by now.
    await page.waitForTimeout(3_000)
    await expect(page).not.toHaveURL(/\/setup\b/)

    // The login form is also forbidden — we should have landed
    // somewhere in the app after the mocked auto_login.
    await expect(
      page.getByRole('textbox', { name: /username/i }),
    ).toHaveCount(0)
  })
})
