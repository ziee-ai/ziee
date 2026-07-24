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

/** Tracks which module chunks the browser has requested. Exposes a `has(name)`
 *  compatible with the previous Set API. Matches BOTH the dev path
 *  (`/modules/.../<name>/module.tsx`) and the prod build chunk this repo emits
 *  (`assets/module.<name>.<hash>.js`, named by rollupOptions.output.chunkFileNames
 *  in vite.config.ts / global-setup) — the e2e runs a prod build, where module
 *  sources are bundled to hashed chunks and the dev path is never requested. */
function trackModuleRequests(page: Page): { has: (name: string) => boolean } {
  const urls: string[] = []
  page.on('request', r => urls.push(r.url()))
  return {
    has: (name: string) =>
      urls.some(
        u =>
          new RegExp(`/modules/(?:[^?]*/)?${name}/module\\.tsx`).test(u) ||
          new RegExp(`/module\\.${name}\\.[A-Za-z0-9_-]+\\.js`).test(u),
      ),
  }
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
    // Let any reactive module-load wave finish. NOTE: a no-perm user can't open
    // the sync SSE stream (profile::read gated), so it retry-churns and
    // `networkidle` never settles — use a bounded settle instead. This asserts a
    // NEGATIVE (no admin module requested); if the gate were broken the request
    // would fire during this initial wave, so a fixed wait is sufficient + robust.
    await page.waitForTimeout(2500)

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
    // Bounded settle (not networkidle — the no-perm user's SSE retry-churns).
    // The route-driven loader runs on nav; if the gate leaked, the `user` module
    // chunk would be requested within this window.
    await page.waitForTimeout(2000)
    expect(
      requested.has('user'),
      'the user-admin module must not load even on direct navigation for a non-admin',
    ).toBe(false)
  })
})
