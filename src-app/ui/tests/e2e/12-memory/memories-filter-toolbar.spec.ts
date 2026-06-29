import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'
import {
  loginAsAdmin,
  getAdminToken,
  createTestUser,
  login,
  getCurrentUserToken,
} from '../../common/auth-helpers'

/**
 * E2E — MyMemoriesSection filter *toolbar* (audit r2-a4de58ae3a68).
 *
 * Distinct from the search-box test (`memories-search.spec.ts`): the toolbar
 * also exposes a "Kind" `<Select>` whose onChange drives
 * `Stores.Memories.setKindFilter` — a SERVER-side exact-match filter
 * (`GET /api/memories?kind=<kind>`). No prior spec exercised it. This seeds
 * two memories of different kinds, picks a kind in the toolbar, and asserts
 * the list filters to that kind via the real fetch.
 */

async function memoryUser(apiURL: string, name: string) {
  const adminToken = await getAdminToken(apiURL)
  const username = `${name}_${Date.now().toString(36)}`
  await createTestUser(
    apiURL,
    adminToken,
    username,
    `${username}@ex.com`,
    'password123',
    ['profile::read', 'profile::edit', 'memory::read', 'memory::write'],
  )
  return username
}

test.describe('Memory — My memories filter toolbar', () => {
  test('selecting a Kind in the toolbar filters the list server-side', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const username = await memoryUser(apiURL, 'mem_filter')
    await login(page, baseURL, username, 'password123')
    const authHeader = {
      Authorization: `Bearer ${await getCurrentUserToken(page)}`,
    }

    // Seed two memories that differ along the toolbar's Kind dimension.
    const factText = 'User was born in the city of QUAXTOWN'
    const goalText = 'User wants to learn the FLIMBERLANG language'
    const seed = [
      { content: factText, kind: 'fact' },
      { content: goalText, kind: 'goal' },
    ]
    const ids: Record<string, string> = {}
    for (const data of seed) {
      const res = await page.request.post(`${apiURL}/api/memories`, {
        headers: authHeader,
        data,
      })
      expect(res.status()).toBe(201)
      ids[data.content] = (await res.json()).id as string
    }

    await page.goto(`${baseURL}/settings/memory`)

    // Both memories render before filtering (targeted by stable data-memory-id).
    await expect(page.locator(`[data-memory-id="${ids[factText]}"]`)).toBeVisible({ timeout: 10_000 })
    await expect(page.locator(`[data-memory-id="${ids[goalText]}"]`)).toBeVisible()

    // Open the "Kind" Select and pick "Goal" — the store fires a server-side
    // GET /api/memories?kind=goal (proves the real filtered fetch).
    const kindReq = page.waitForRequest(
      req =>
        req.url().includes('/api/memories') &&
        /[?&]kind=goal/.test(decodeURIComponent(req.url())),
    )
    await byTestId(page, 'memory-kind-filter').click()
    await byTestId(page, 'memory-kind-filter-opt-goal').click()
    await kindReq

    // Only the goal memory remains; the fact memory is filtered out.
    await expect(page.locator(`[data-memory-id="${ids[goalText]}"]`)).toBeVisible({ timeout: 10_000 })
    await expect(page.locator(`[data-memory-id="${ids[factText]}"]`)).toHaveCount(0)
  })
})
