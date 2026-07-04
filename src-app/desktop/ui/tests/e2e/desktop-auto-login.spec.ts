import { test, expect } from '@playwright/test'
import {
  FAKE_TOKENS,
  installTauriMock,
  mockBackendDefaults,
} from './helpers/tauri-mock'

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
    await expect(page.getByTestId('desktop-bootstrap-starting')).toBeVisible({
      timeout: 5_000,
    })

    // After ~3.5 s the third call succeeds → spinner gone, no AuthPage.
    await expect(page.getByTestId('desktop-bootstrap-starting')).toBeHidden({
      timeout: 10_000,
    })
    await expect(
      page.getByRole('textbox', { name: /username/i }),
    ).toHaveCount(0)

    const calls = await page.evaluate(
      () => (window as any).__TAURI_MOCK_CALLS__.auto_login,
    )
    expect(calls).toBeGreaterThanOrEqual(3)
  })

  test('refresh failure falls back to auto_login — the desktop session is permanent', async ({
    page,
  }) => {
    // Short-lived token so the shared Auth store's proactive silent
    // refresh fires within the test (75% of 4s ≈ 3s)…
    await installTauriMock(page, {
      autoLogin: 'success',
      tokens: { ...FAKE_TOKENS, expires_in: 4 },
    })
    await mockBackendDefaults(page)
    // …and the refresh endpoint hard-401s (revoked/expired session —
    // e.g. the machine slept past the full session length). The desktop
    // fallback must re-mint via auto_login instead of logging out.
    await page.route('**/api/auth/refresh', async route => {
      await route.fulfill({
        status: 401,
        contentType: 'application/json',
        body: JSON.stringify({
          error: 'revoked',
          error_code: 'REFRESH_TOKEN_REVOKED',
        }),
      })
    })

    await page.goto('/')

    // Bootstrap lands normally on the first auto_login.
    await expect(
      page.getByRole('textbox', { name: /username/i }),
    ).toHaveCount(0)

    // Wait past the refresh point (~3s) + fallback round-trip, then
    // assert the fallback re-minted locally: a SECOND auto_login call…
    await expect
      .poll(
        () =>
          page.evaluate(
            () => (window as any).__TAURI_MOCK_CALLS__.auto_login,
          ),
        { timeout: 15_000 },
      )
      .toBeGreaterThanOrEqual(2)

    // …and the user NEVER saw a login surface or an error screen.
    await expect(
      page.getByRole('textbox', { name: /username/i }),
    ).toHaveCount(0)
    await expect(page).not.toHaveURL(/\/auth\b/)
    await expect(page.getByTestId('desktop-bootstrap-failed')).toHaveCount(0)
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
    await expect(page.getByTestId('desktop-bootstrap-failed')).toBeVisible({
      timeout: 35_000,
    })
    // Even on failure the AuthPage must NOT appear — desktop has no
    // recoverable login form to offer.
    await expect(
      page.getByRole('textbox', { name: /username/i }),
    ).toHaveCount(0)
  })
})
