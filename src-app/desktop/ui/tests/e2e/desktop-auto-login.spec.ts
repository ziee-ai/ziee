import { test, expect } from '@playwright/test'
import { installTauriMock, mockBackendDefaults } from './helpers/tauri-mock'

test.describe('desktop auto-login', () => {
  test('happy path: lands on app without flashing the login form', async ({
    page,
  }) => {
    await installTauriMock(page, { autoLogin: 'success' })
    await mockBackendDefaults(page)

    await page.goto('/')

    // Wait for the spinner caption to either disappear or for the URL
    // to settle off any auth-related path. The desktop AuthGuard never
    // shows AuthPage, so an absent username textbox proves it.
    await expect(
      page.getByRole('textbox', { name: /username/i }),
    ).toHaveCount(0)
    await expect(page).not.toHaveURL(/\/auth\b/)
    await expect(page).not.toHaveURL(/\/setup\b/)

    // Exactly one auto_login call on the happy path.
    const calls = await page.evaluate(
      () => (window as any).__TAURI_MOCK_CALLS__.auto_login,
    )
    expect(calls).toBe(1)
  })

  test('retry path: 2 failures then success keeps spinner up, never AuthPage', async ({
    page,
  }) => {
    await installTauriMock(page, { autoLogin: { failFirstN: 2 } })
    await mockBackendDefaults(page)

    await page.goto('/')

    // While the retries happen, the spinner caption is visible. Wait
    // long enough for two backoff cycles (500ms + 1s + retry budget).
    await expect(page.getByText(/starting up/i)).toBeVisible({ timeout: 5_000 })

    // After ~3.5 s the third call succeeds → spinner gone, no AuthPage.
    await expect(page.getByText(/starting up/i)).toBeHidden({ timeout: 10_000 })
    await expect(
      page.getByRole('textbox', { name: /username/i }),
    ).toHaveCount(0)

    const calls = await page.evaluate(
      () => (window as any).__TAURI_MOCK_CALLS__.auto_login,
    )
    expect(calls).toBeGreaterThanOrEqual(3)
  })

  test('permanent failure: spinner switches to the actionable message after the deadline', async ({
    page,
  }) => {
    await installTauriMock(page, { autoLogin: 'fail-forever' })
    await mockBackendDefaults(page)

    await page.goto('/')

    // 30 s wall-clock budget — bump the test timeout accordingly. The
    // failure message is the contract the user sees if the embedded
    // server never recovers.
    await expect(page.getByText(/backend failed to start/i)).toBeVisible({
      timeout: 35_000,
    })
    // Even on failure the AuthPage must NOT appear — desktop has no
    // recoverable login form to offer.
    await expect(
      page.getByRole('textbox', { name: /username/i }),
    ).toHaveCount(0)
  })
})
