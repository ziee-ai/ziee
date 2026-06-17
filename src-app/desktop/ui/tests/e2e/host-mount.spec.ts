/**
 * Host-mount desktop UI E2E (desktop bundle).
 *
 * Asserts the desktop-only host-mount surfaces render and round-trip against
 * their `/api/host-mounts/*` routes: the admin policy page (toggle + save) and
 * the settings-sidebar registration.
 *
 * Backend is mocked (`installTauriMock` + `mockBackendDefaults` + per-route
 * stubs) — the real desktop routes are exercised by the Rust integration
 * tests; this spec is for UI correctness + the desktop registration/gating.
 */

import { test, expect } from '@playwright/test'
import { installTauriMock, mockBackendDefaults } from './helpers/tauri-mock'

// A full admin MeResponse. `setAuthFromAutoLogin` seeds `user` but NOT
// `permissions`, and AuthGuard.initAuth() re-fetches `/api/auth/me` right
// after — which the `mockBackendDefaults` catch-all would otherwise answer
// with `[]`, wiping the admin user. Stubbing `/auth/me` explicitly keeps
// `user.is_admin` + `['*']` so the `host_mount::manage` route gate passes.
const ADMIN_ME = {
  has_password: true,
  permissions: ['*'],
  user: {
    id: '00000000-0000-0000-0000-000000000001',
    username: 'admin',
    email: 'admin@localhost',
    email_verified: true,
    is_active: true,
    is_admin: true,
    permissions: ['*'],
    completed_onboarding_ids: ['getting-started'],
    completed_onboarding_step_ids: [],
    created_at: '2026-01-01T00:00:00Z',
    updated_at: '2026-01-01T00:00:00Z',
  },
}

test.describe('desktop host-mount settings', () => {
  test.beforeEach(async ({ page }) => {
    await installTauriMock(page)
    await mockBackendDefaults(page)
    // Registered AFTER the catch-all so it takes priority (Playwright
    // matches routes most-recently-added first).
    await page.route('**/api/auth/me', async (route) =>
      route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify(ADMIN_ME),
      }),
    )
  })

  test('admin policy page renders, toggles, and saves', async ({ page }) => {
    let savedBody: Record<string, unknown> | null = null
    await page.route('**/api/host-mounts/policy', async (route) => {
      if (route.request().method() === 'PUT') {
        savedBody = route.request().postDataJSON()
        await route.fulfill({
          status: 200,
          contentType: 'application/json',
          body: JSON.stringify({
            enabled: savedBody!.enabled,
            allowed_prefixes: savedBody!.allowed_prefixes ?? [],
            allow_readwrite: savedBody!.allow_readwrite,
          }),
        })
      } else {
        await route.fulfill({
          status: 200,
          contentType: 'application/json',
          body: JSON.stringify({
            enabled: true,
            allowed_prefixes: [],
            allow_readwrite: false,
          }),
        })
      }
    })

    // Reach the page the way a user does: open Settings, click the entry.
    await page.goto('/settings')
    await page.getByText(/Host Mount Policy/i).first().click()

    await expect(page.getByText(/Allow host-folder mounting/i)).toBeVisible({
      timeout: 15_000,
    })
    await expect(page.getByText(/Allowed path prefixes/i)).toBeVisible()

    // Toggling read-write enables Save; saving sends the new policy.
    await page.getByRole('switch').nth(1).click()
    const save = page.getByRole('button', { name: 'Save' })
    await expect(save).toBeEnabled()
    await save.click()

    await expect.poll(() => savedBody).not.toBeNull()
    expect(savedBody!.allow_readwrite).toBe(true)
    expect(savedBody!.enabled).toBe(true)
  })

  test('host-mount policy appears in the admin settings sidebar', async ({ page }) => {
    await page.route('**/api/host-mounts/policy', async (route) =>
      route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({ enabled: true, allowed_prefixes: [], allow_readwrite: false }),
      }),
    )

    await page.goto('/settings')
    // The desktop-only module registers a "Host Mount Policy" admin entry.
    await expect(page.getByText(/Host Mount Policy/i).first()).toBeVisible({
      timeout: 15_000,
    })
  })
})
