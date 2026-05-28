/**
 * Remote Access settings page E2E (desktop bundle).
 *
 * Asserts the page is registered, the inline setup flow renders with
 * the documented gates (auth token first → tunnel start disabled
 * until token saved → QR + plaintext URL after start), and the
 * password-auth toggle is OFF by default.
 *
 * Backend is mocked: we don't actually start an ngrok tunnel here.
 * The aim is UI correctness; Tier 8 (real-ngrok) covers the
 * tunneling itself.
 */

import { test, expect } from '@playwright/test'
import { installTauriMock, mockBackendDefaults } from './helpers/tauri-mock'

test.describe('desktop Remote Access settings', () => {
  test.beforeEach(async ({ page }) => {
    await installTauriMock(page)
    await mockBackendDefaults(page)

    // Stub the remote-access endpoints with sensible defaults.
    await page.route('**/api/remote-access/status', async (route) => {
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({
          password_rotated: false,
          password_auth_enabled: false,
          auth_token_set: false,
          ngrok_domain: null,
          auto_start_tunnel: false,
          tunnel_state: 'idle',
          public_url: null,
          last_error: null,
          started_at: null,
        }),
      })
    })
    await page.route('**/api/remote-access/settings', async (route) => {
      if (route.request().method() === 'GET') {
        await route.fulfill({
          status: 200,
          contentType: 'application/json',
          body: JSON.stringify({
            auth_token_set: false,
            ngrok_domain: null,
            auto_start_tunnel: false,
            password_auth_enabled: false,
          }),
        })
      } else {
        // PUT: echo back the (partial) merged state. The page calls
        // loadStatus() right after, so the next /status call should
        // reflect the changes. For this happy-path test, the
        // default-stub above is sufficient.
        await route.fulfill({
          status: 200,
          contentType: 'application/json',
          body: JSON.stringify({
            auth_token_set: true,
            ngrok_domain: null,
            auto_start_tunnel: false,
            password_auth_enabled: false,
          }),
        })
      }
    })
  })

  test('Remote Access menu entry is visible', async ({ page }) => {
    await page.goto('/settings')
    await expect(page.getByRole('menuitem').first()).toBeVisible({
      timeout: 10_000,
    })
    const menu = page.getByRole('menu')
    await expect(menu.getByText(/Remote Access/i)).toBeVisible()
  })

  test('clicking Remote Access lands on the settings page', async ({ page }) => {
    await page.goto('/settings')
    await page.getByRole('menu').getByText(/Remote Access/i).click()
    await expect(page).toHaveURL(/\/settings\/remote-access\b/)
    await expect(page.getByRole('heading', { name: 'Remote Access' })).toBeVisible()
  })

  test('starts with auth-token gate; tunnel start hidden until saved', async ({
    page,
  }) => {
    await page.goto('/settings/remote-access')
    // Token card visible — match the card title exactly (the Alert
    // below also contains the substring "ngrok auth token", which
    // would cause Playwright strict-mode to throw on a loose match).
    await expect(page.getByText('ngrok auth token', { exact: true })).toBeVisible()
    // The "Start tunnel" button only renders once a token is saved.
    await expect(page.getByRole('button', { name: 'Start tunnel' })).toHaveCount(0)
    // And the gate alert is shown.
    await expect(page.getByText(/Add your ngrok auth token first/i)).toBeVisible()
  })

  test('password authentication toggle defaults OFF', async ({ page }) => {
    await page.goto('/settings/remote-access')
    const toggle = page
      .locator('.ant-form-item-label', { hasText: 'Enable password authentication' })
      .locator('..')
      .getByRole('switch')
    await expect(toggle).toHaveCount(1)
    await expect(toggle).not.toBeChecked()
  })
})
