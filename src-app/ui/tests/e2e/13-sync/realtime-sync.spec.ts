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

  test("a project created by user A is NOT delivered to user B's stream", async ({
    page,
    browser,
    testInfra,
  }) => {
    const { baseURL } = testInfra

    // A second, distinct user with just enough to subscribe + use projects.
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

    // User A = admin (device A).
    await loginAsAdmin(page, baseURL)
    await goToProjectsPage(page, baseURL)

    // User B = the other user (separate context), with a LIVE sync stream.
    const ctxB = await browser.newContext()
    const pageB = await ctxB.newPage()
    try {
      await login(pageB, baseURL, username, password)
      await goToProjectsPage(pageB, baseURL)

      const name = `Isolation E2E ${uniq}`

      // User A creates a project (Owner(admin)).
      await openCreateProjectDrawer(page)
      await fillProjectForm(page, { name })
      await submitProjectForm(page)

      // Give the (incorrect) cross-user delivery a chance to happen, then
      // assert user B never saw it. B's own list stays empty of A's project.
      await pageB.waitForTimeout(3_000)
      await expect(getProjectCard(pageB, name)).not.toBeVisible()
    } finally {
      await ctxB.close()
    }
  })
})
