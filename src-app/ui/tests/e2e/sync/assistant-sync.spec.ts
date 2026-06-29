import { test, expect } from '../../fixtures/test-context'
import {
  loginAsAdmin,
  login,
  createTestUser,
  getAdminToken,
} from '../../common/auth-helpers'
import {
  goToUserAssistantsPage,
  openCreateAssistantDrawer,
  fillAssistantForm,
  submitAssistantForm,
  getUserAssistantRow,
} from '../assistants/helpers/assistant-helpers'

// Realtime sync for the `assistant` entity (Owner-scoped): a user's assistant
// reaches the SAME user's other device live, and never a different user.
// Run with --workers=1 (shared backend + DB).
test.describe('Realtime sync — assistant (owner-scoped)', () => {
  test('an assistant created on device A appears on the same user device B without reload', async ({
    page,
    browser,
    testInfra,
  }) => {
    const { baseURL } = testInfra

    await loginAsAdmin(page, baseURL)
    await goToUserAssistantsPage(page, baseURL)

    const ctxB = await browser.newContext()
    const pageB = await ctxB.newPage()
    try {
      await loginAsAdmin(pageB, baseURL)
      await goToUserAssistantsPage(pageB, baseURL)

      const name = `Sync Assistant ${Date.now()}`
      await openCreateAssistantDrawer(page)
      await fillAssistantForm(page, { name })
      await submitAssistantForm(page)

      // Device B must show it WITHOUT a manual reload — the SSE sync event
      // makes the assistants store refetch. Playwright auto-waits.
      await expect(await getUserAssistantRow(pageB, name)).toBeVisible({ timeout: 15_000 })
    } finally {
      await ctxB.close()
    }
  })

  test("an assistant reaches the owner's other device but NOT a different user", async ({
    page,
    browser,
    testInfra,
  }) => {
    const { baseURL } = testInfra

    // Create the admin first (loginAsAdmin onboards on the fresh per-test
    // backend) so getAdminToken below can authenticate.
    await loginAsAdmin(page, baseURL)
    await goToUserAssistantsPage(page, baseURL)

    const adminToken = await getAdminToken(baseURL)
    const uniq = Date.now()
    const username = `asst_other_${uniq}`
    const password = 'Password123!'
    await createTestUser(
      baseURL,
      adminToken,
      username,
      `${username}@example.com`,
      password,
      ['profile::read', 'assistants::read', 'assistants::create'],
    )

    const ctxA2 = await browser.newContext() // owner, device 2 — positive control
    const pageA2 = await ctxA2.newPage()
    const ctxB = await browser.newContext() // different user — isolation
    const pageB = await ctxB.newPage()
    try {
      await loginAsAdmin(pageA2, baseURL)
      await goToUserAssistantsPage(pageA2, baseURL)
      await login(pageB, baseURL, username, password)
      await goToUserAssistantsPage(pageB, baseURL)

      const name = `Isolation Assistant ${uniq}`
      await openCreateAssistantDrawer(page)
      await fillAssistantForm(page, { name })
      await submitAssistantForm(page)

      // Positive control: the owner's OTHER device receives it live.
      await expect(await getUserAssistantRow(pageA2, name)).toBeVisible({ timeout: 15_000 })
      // Isolation: a different user (same delivery window) never sees it.
      await expect(await getUserAssistantRow(pageB, name)).not.toBeVisible()
    } finally {
      await ctxA2.close()
      await ctxB.close()
    }
  })
})
