import { test, expect } from '../../fixtures/test-context'
import {
  loginAsAdmin,
  getAdminToken,
  createTestUser,
  login,
  getCurrentUserToken,
} from '../../common/auth-helpers'

/**
 * E2E — MyMemoriesSection search box (audit gap all-f62a9032e5bf).
 *
 * MyMemoriesSection.tsx renders an antd `<Search placeholder="Search
 * content">` whose onChange drives `Stores.Memories.setSearchQuery`
 * (Memories.store.ts:250-258) — a 250ms-debounced, SERVER-side filter
 * (`GET /api/memories?search=<q>`). No prior spec typed into that box;
 * this seeds two memories with distinct content, searches for one, and
 * asserts the list filters down to the match (the other disappears),
 * proving the search action drives the filtered fetch end-to-end.
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

test.describe('Memory — My memories search', () => {
  test('typing in the search box filters the list to the match', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const username = await memoryUser(apiURL, 'mem_search')
    await login(page, baseURL, username, 'password123')
    const authHeader = {
      Authorization: `Bearer ${await getCurrentUserToken(page)}`,
    }

    // Seed two memories with disjoint, unique content tokens.
    const matchText = 'User prefers the ZEBRACORN editor for everything'
    const otherText = 'User commutes by WALRUSBOAT on weekdays'
    for (const content of [matchText, otherText]) {
      const res = await page.request.post(`${apiURL}/api/memories`, {
        headers: authHeader,
        data: { content, kind: 'fact' },
      })
      expect(res.status()).toBe(201)
    }

    await page.goto(`${baseURL}/settings/memory`)

    // Both memories render before filtering.
    await expect(page.getByText(matchText)).toBeVisible({ timeout: 10_000 })
    await expect(page.getByText(otherText)).toBeVisible()

    // Search for a token unique to the first memory. The store fires a
    // debounced GET /api/memories?search=ZEBRACORN — assert it goes to
    // the server with the query (proves the real filtered fetch, not a
    // client-only filter).
    const searchReq = page.waitForRequest(
      req =>
        req.url().includes('/api/memories') &&
        /[?&]search=ZEBRACORN/.test(decodeURIComponent(req.url())),
    )
    await page.getByPlaceholder('Search content').fill('ZEBRACORN')
    await searchReq

    // The matching memory stays; the non-matching one is filtered out.
    await expect(page.getByText(matchText)).toBeVisible({ timeout: 10_000 })
    await expect(page.getByText(otherText)).toHaveCount(0)
  })
})
