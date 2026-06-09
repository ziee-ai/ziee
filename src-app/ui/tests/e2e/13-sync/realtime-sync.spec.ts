import { test, expect } from '../../fixtures/test-context'
import {
  loginAsAdmin,
  login,
  createTestUser,
  getAdminToken,
} from '../../common/auth-helpers'
import {
  goToProjectsPage,
  openCreateProjectDrawer,
  fillProjectForm,
  submitProjectForm,
  getProjectCard,
} from '../11-projects/helpers/project-helpers'

// Realtime cross-device sync, exercised end-to-end through the projects
// surface (Owner-scoped sync). Run with --workers=1 (shared backend + DB).
//
// These prove the two guarantees that matter most:
//   1. a change on one device appears on the SAME user's other device live;
//   2. a change is NEVER delivered to a DIFFERENT user.
test.describe('Realtime sync (cross-device)', () => {
  test('a project created on device A appears on device B without reload', async ({
    page,
    browser,
    testInfra,
  }) => {
    const { baseURL } = testInfra

    // Device A
    await loginAsAdmin(page, baseURL)
    await goToProjectsPage(page, baseURL)

    // Device B — a second browser context for the SAME admin user.
    const ctxB = await browser.newContext()
    const pageB = await ctxB.newPage()
    try {
      await loginAsAdmin(pageB, baseURL)
      await goToProjectsPage(pageB, baseURL)

      const name = `Sync E2E ${Date.now()}`

      // Create on device A.
      await openCreateProjectDrawer(page)
      await fillProjectForm(page, { name })
      await submitProjectForm(page)

      // Device B must show it WITHOUT a manual reload — the SSE sync event
      // triggers the projects list to refetch. Playwright auto-waits.
      await expect(getProjectCard(pageB, name)).toBeVisible({ timeout: 15_000 })
    } finally {
      await ctxB.close()
    }
  })

  test("user A's project reaches A's other device but NOT a different user B", async ({
    page,
    browser,
    testInfra,
  }) => {
    const { baseURL } = testInfra

    // User A = admin, device 1. loginAsAdmin onboards the admin on this test's
    // fresh backend FIRST, so getAdminToken below can authenticate.
    await loginAsAdmin(page, baseURL)
    await goToProjectsPage(page, baseURL)

    // A second, distinct user. createTestUser auto-joins the default Users
    // group, so B gets a working app shell + a live sync stream.
    const adminToken = await getAdminToken(baseURL)
    const uniq = Date.now()
    const username = `sync_other_${uniq}`
    const password = 'Password123!'
    await createTestUser(
      baseURL,
      adminToken,
      username,
      `${username}@example.com`,
      password,
      ['profile::read', 'projects::read', 'projects::create'],
    )

    const ctxA2 = await browser.newContext() // User A, device 2 — positive control
    const pageA2 = await ctxA2.newPage()
    const ctxB = await browser.newContext() // User B — isolation
    const pageB = await ctxB.newPage()
    try {
      await loginAsAdmin(pageA2, baseURL)
      await goToProjectsPage(pageA2, baseURL)
      await login(pageB, baseURL, username, password)
      await goToProjectsPage(pageB, baseURL)

      const name = `Isolation E2E ${uniq}`

      // User A (device 1) creates a project (Owner(admin)).
      await openCreateProjectDrawer(page)
      await fillProjectForm(page, { name })
      await submitProjectForm(page)

      // Positive control: A's OTHER device receives it live — proves the sync
      // event actually fired + was delivered (so B's absence below is
      // meaningful, not just "sync is dead").
      await expect(getProjectCard(pageA2, name)).toBeVisible({ timeout: 15_000 })

      // Isolation: user B had the SAME delivery window (A2 already received
      // it) yet must never see user A's project. No fixed sleep needed — the
      // positive control above is the synchronization point.
      await expect(getProjectCard(pageB, name)).not.toBeVisible()
    } finally {
      await ctxA2.close()
      await ctxB.close()
    }
  })
})
