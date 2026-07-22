import { test, expect } from '../../fixtures/test-context'
import type { Page } from '@playwright/test'
import {
  loginAsAdmin,
  getAdminToken,
  createTestUser,
  login,
} from '../../common/auth-helpers'

/**
 * E2E — SMART MODULE LOADING (build-manifest + shouldLoad gating).
 *
 * Modules are no longer all-loaded at boot. A build-generated manifest carries
 * each module's `shouldLoad` predicate; the loader downloads a module's body
 * only when it's eligible (auth + permission). These specs assert the observable
 * consequences over the REAL stack (dev server ⇒ each module.tsx is its own
 * request, so we can observe exactly which modules a browser downloaded).
 */

/** Names of module.tsx files the browser has requested so far. */
function trackModuleRequests(page: Page): Set<string> {
  const seen = new Set<string>()
  page.on('request', r => {
    const m = r.url().match(/\/modules\/(?:.*\/)?([a-z0-9_-]+)\/module\.tsx/)
    if (m) seen.add(m[1])
  })
  return seen
}

const CORE = ['app', 'auth', 'config-client', 'router']
// Representative admin-only modules — gated on a permission a fresh user lacks.
const ADMIN_ONLY = ['user', 'code-sandbox', 'hardware', 'llm-provider']

test.describe('smart module loading', () => {
  test('the unauthenticated login page downloads only core modules, no feature modules', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    const requested = trackModuleRequests(page)

    await page.goto(`${baseURL}/login`)
    // Let the login page settle.
    await page.waitForLoadState('networkidle')

    // Core modules the login/setup surface depends on are present…
    for (const c of CORE) {
      expect(requested.has(c), `core module "${c}" must load`).toBe(true)
    }
    // …and NO authenticated feature module was downloaded pre-auth.
    for (const feat of ['chat', 'mcp', 'projects', 'file', ...ADMIN_ONLY]) {
      expect(
        requested.has(feat),
        `feature module "${feat}" must NOT load before authentication`,
      ).toBe(false)
    }
  })

  test('after login the authenticated app registers its modules (reactive wave)', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    const requested = trackModuleRequests(page)

    await loginAsAdmin(page, baseURL)

    // The authenticated shell renders (the reactive wave registered the modules
    // that fill it) — the app root + primary chat surface are reachable.
    await expect(page.locator('[data-testid="app-root"]')).toBeVisible({
      timeout: 15000,
    })
    // Chat is an authenticated module — its body must have been downloaded.
    expect(
      requested.has('chat'),
      'chat module loads once authenticated',
    ).toBe(true)
  })

  test('[negative-perm] a user lacking a permission never downloads that admin module, and its route is denied', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra

    // A fresh non-admin user with NO permissions.
    const adminToken = await getAdminToken(apiURL)
    await createTestUser(
      apiURL,
      adminToken,
      'noperm',
      'noperm@test.local',
      'NoPermUser12345',
      [],
    )

    const requested = trackModuleRequests(page)
    await login(page, baseURL, 'noperm', 'NoPermUser12345', {
      completeOnboarding: true,
    })
    await expect(page.locator('[data-testid="app-root"]')).toBeVisible({
      timeout: 15000,
    })
    // Let any reactive wave finish.
    await page.waitForLoadState('networkidle')

    // The admin-only module bodies were NEVER downloaded to this browser —
    // permission gating happens at DOWNLOAD time, the strongest possible gate.
    for (const admin of ADMIN_ONLY) {
      expect(
        requested.has(admin),
        `admin module "${admin}" must NOT be downloaded for a non-admin`,
      ).toBe(false)
    }

    // And the admin route is not reachable (fail-closed): navigating to the user
    // admin page does not render its page — the module owning it never loaded,
    // so the route falls through the guard.
    await page.goto(`${baseURL}/settings/users`)
    await page.waitForLoadState('networkidle')
    expect(
      requested.has('user'),
      'the user-admin module must not load even on direct navigation for a non-admin',
    ).toBe(false)
  })
})
