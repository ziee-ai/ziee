import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'
import type { Page } from '@playwright/test'
import {
  loginAsAdmin,
  login,
  createTestUser,
  getAdminToken,
} from '../../common/auth-helpers'

/**
 * Realtime sync for the MEMORY module. Three entities:
 *
 *  - `memory`               (OWNER-scoped)      — /settings/memory list
 *  - `memory_settings`      (OWNER-scoped)      — /settings/memory prefs
 *  - `memory_admin_settings`(PERM memory::admin::read) — /settings/memory-admin
 *
 * Each store subscribes to its `sync:<entity>` event and refetches, so a
 * mutation on device A surfaces on the SAME user's device B (or admin's
 * other device, for the deployment-wide singleton) WITHOUT a manual reload.
 * Cross-user / cross-permission isolation is asserted in the same delivery
 * window (a negative wait alongside a positive control).
 *
 * Run with --workers=1 (shared backend + DB).
 *
 * NOTE on navigation: the realtime-sync SSE stream is a persistent
 * connection that keeps the network busy, so `waitForLoadState('networkidle')`
 * never settles and hangs. Every nav here is an inline `page.goto(...)`
 * followed by a wait on a stable page selector — never a networkidle helper.
 */

// `/settings/memory` is stable once the per-user Preferences card's
// switches render (the page route is gated on memory::read). For a
// memory::write user the "Add memory" CTA is also present.
async function goToMemoryPage(page: Page, baseURL: string) {
  await page.goto(`${baseURL}/settings/memory`)
  await page.getByRole('switch').first().waitFor({ state: 'visible', timeout: 15_000 })
}

// `/settings/memory-admin` is stable once the Engine card's lone switch
// ("Enable memory deployment-wide") renders. Admin-only.
async function goToMemoryAdminPage(page: Page, baseURL: string) {
  await page.goto(`${baseURL}/settings/memory-admin`)
  await page.getByRole('switch').first().waitFor({ state: 'visible', timeout: 15_000 })
}

// Add a memory through the /settings/memory "My memories" card: open the
// drawer, fill Content, submit. Mirrors tests/e2e/12-memory/manual-add.spec.ts.
async function addMemoryViaUI(page: Page, content: string) {
  await byTestId(page, 'memory-add-btn').click()
  const dialog = page.getByTestId('memory-create-form')
  await expect(dialog).toBeVisible()
  await byTestId(dialog, 'memory-create-content-input').fill(content)
  await byTestId(dialog, 'memory-create-submit-btn').click()
  // Success closes the create dialog.
  await expect(byTestId(page, 'memory-create-form')).toHaveCount(0, { timeout: 5_000 })
}

// A memory row renders as a `[data-memory-id]` div containing its content
// text — the same selector tests/e2e/12-memory uses.
function memoryRow(page: Page, content: string) {
  return page.locator('[data-memory-id]').filter({ hasText: content })
}

// ───────────────────────── memory (owner-scoped) ─────────────────────────
test.describe('Realtime sync — memory (owner-scoped)', () => {
  test('a memory created on device A appears on the same user device B without reload', async ({
    page,
    browser,
    testInfra,
  }) => {
    const { baseURL } = testInfra

    // Admin (root) has memory::read/write via the `*` wildcard — use it as
    // the owner so we don't need a second user for the positive path.
    await loginAsAdmin(page, baseURL)
    await goToMemoryPage(page, baseURL)

    const ctxB = await browser.newContext()
    const pageB = await ctxB.newPage()
    try {
      await loginAsAdmin(pageB, baseURL)
      await goToMemoryPage(pageB, baseURL)

      const content = `Sync memory ${Date.now()}`
      await addMemoryViaUI(page, content)

      // Device B must show it WITHOUT a manual reload — the SSE `sync:memory`
      // event makes the Memories store reload the current page.
      await expect(memoryRow(pageB, content)).toBeVisible({ timeout: 15_000 })
    } finally {
      await ctxB.close()
    }
  })

  test("a memory reaches the owner's other device but NOT a different user", async ({
    page,
    browser,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra

    // Create the admin first (loginAsAdmin onboards on the fresh per-test
    // backend) so getAdminToken below can authenticate. The admin is the
    // memory owner (devices A + A2); a separate user is the isolation probe.
    await loginAsAdmin(page, baseURL)
    await goToMemoryPage(page, baseURL)

    const adminToken = await getAdminToken(apiURL)
    const uniq = Date.now()
    const username = `mem_other_${uniq}`
    const password = 'password123'
    await createTestUser(
      apiURL,
      adminToken,
      username,
      `${username}@ex.com`,
      password,
      ['profile::read', 'profile::edit', 'memory::read', 'memory::write'],
    )

    const ctxA2 = await browser.newContext() // owner, device 2 — positive control
    const pageA2 = await ctxA2.newPage()
    const ctxB = await browser.newContext() // different user — isolation
    const pageB = await ctxB.newPage()
    try {
      await loginAsAdmin(pageA2, baseURL)
      await goToMemoryPage(pageA2, baseURL)
      await login(pageB, baseURL, username, password)
      await goToMemoryPage(pageB, baseURL)

      const content = `Isolation memory ${uniq}`
      await addMemoryViaUI(page, content)

      // Positive control: the owner's OTHER device receives it live.
      await expect(memoryRow(pageA2, content)).toBeVisible({ timeout: 15_000 })
      // Isolation: a different user (same delivery window) never sees it.
      await expect(memoryRow(pageB, content)).not.toBeVisible()
    } finally {
      await ctxA2.close()
      await ctxB.close()
    }
  })
})

// ─────────────────────── memory_settings (owner-scoped) ───────────────────────
test.describe('Realtime sync — memory_settings (owner-scoped)', () => {
  test('a memory preference toggled on device A is reflected on the same user device B', async ({
    page,
    browser,
    testInfra,
  }) => {
    const { baseURL } = testInfra

    await loginAsAdmin(page, baseURL)
    await goToMemoryPage(page, baseURL)

    const ctxB = await browser.newContext()
    const pageB = await ctxB.newPage()
    try {
      await loginAsAdmin(pageB, baseURL)
      await goToMemoryPage(pageB, baseURL)

      // The Preferences card's FIRST switch is "Auto-extract memories"
      // (extraction_enabled). It is OFF by default; flip it ON on device A,
      // then Save (the form persists + emits memory_settings on submit).
      const switchA = byTestId(page, 'memory-prefs-extraction-switch')
      const switchB = byTestId(pageB, 'memory-prefs-extraction-switch')

      // Establish the baseline on B before mutating on A.
      await expect(switchB).toHaveAttribute('aria-checked', 'false')

      await switchA.click()
      await expect(switchA).toHaveAttribute('aria-checked', 'true')
      // Scope Save to the Preferences card so a future second form on the
      // page can't shadow the right button (there are several "Save"s app-wide).
      const _prefsSaved = page.waitForResponse(
        r => /\/api\/memory\/settings$/.test(r.url()) && r.request().method() === 'PUT',
      )
      await byTestId(page, 'memory-prefs-save-btn').click()
      await _prefsSaved

      // Device B reflects the new setting WITHOUT a reload — the
      // `sync:memory_settings` event reloads the singleton and the form's
      // effect re-syncs the controlled Switch.
      await expect(switchB).toHaveAttribute('aria-checked', 'true', {
        timeout: 15_000,
      })
    } finally {
      await ctxB.close()
    }
  })
})

// ───────────────── memory_admin_settings (permission: memory::admin::read) ─────────────────
test.describe('Realtime sync — memory_admin_settings (permission-scoped)', () => {
  test('toggling deployment memory on device A reflects on admin device B but not a non-admin', async ({
    page,
    browser,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra

    // Admin first (onboards) so getAdminToken/createTestUser work after.
    await loginAsAdmin(page, baseURL)
    await goToMemoryAdminPage(page, baseURL)

    const adminToken = await getAdminToken(apiURL)
    const uniq = Date.now()
    // A non-admin user with only the regular memory perms — no
    // memory::admin::read, so the admin page's Engine switch never renders.
    const username = `mem_nonadmin_${uniq}`
    const password = 'password123'
    await createTestUser(
      apiURL,
      adminToken,
      username,
      `${username}@ex.com`,
      password,
      ['profile::read', 'profile::edit', 'memory::read', 'memory::write'],
    )

    const ctxAdminB = await browser.newContext() // admin, device 2 — positive
    const pageAdminB = await ctxAdminB.newPage()
    const ctxUser = await browser.newContext() // non-admin — isolation
    const pageUser = await ctxUser.newPage()
    try {
      await loginAsAdmin(pageAdminB, baseURL)
      await goToMemoryAdminPage(pageAdminB, baseURL)

      // The non-admin can't reach the admin page surface; assert the
      // deployment-enable switch is absent for them (route + section gating).
      await login(pageUser, baseURL, username, password)
      await pageUser.goto(`${baseURL}/settings/memory-admin`)
      await pageUser.waitForLoadState('load')

      // The Engine card's lone switch is "Enable memory deployment-wide".
      const enableA = byTestId(page, 'memory-admin-enabled-switch')
      const enableAdminB = byTestId(pageAdminB, 'memory-admin-enabled-switch')

      // Baseline: memory is OFF deployment-wide by default.
      await expect(enableAdminB).toHaveAttribute('aria-checked', 'false')

      await enableA.click()
      await expect(enableA).toHaveAttribute('aria-checked', 'true')
      // The admin page stacks four section forms each with its own "Save";
      // scope to the Engine card (the one holding the lone switch) so we
      // submit the right one.
      const _engineSaved = page.waitForResponse(
        r => /\/api\/memory\/admin-settings$/.test(r.url()) && r.request().method() === 'PUT',
      )
      await byTestId(page, 'memory-admin-master-save-btn').click()
      await _engineSaved

      // Positive: the admin's OTHER device reflects the deployment toggle
      // live via `sync:memory_admin_settings`.
      await expect(enableAdminB).toHaveAttribute('aria-checked', 'true', {
        timeout: 15_000,
      })

      // Isolation: the non-admin never sees a deployment-enable switch at
      // all (no memory::admin::read → the admin route/section is gated off).
      await expect(pageUser.getByRole('switch')).toHaveCount(0)
    } finally {
      await ctxAdminB.close()
      await ctxUser.close()
    }
  })
})
