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
 * E2E — MyMemoriesSection search box (audit gap all-f62a9032e5bf).
 *
 * MyMemoriesSection renders a search `<Input placeholder="Search content">`
 * whose onChange drives `Stores.Memories.setSearchQuery` — a 250ms-debounced,
 * SERVER-side filter (`GET /api/memories?search=<q>`). No prior spec typed
 * into that box; this seeds two memories with distinct content, searches for
 * one, and asserts the list filters down to the match (the other disappears),
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

    // Seed two memories with disjoint, unique content tokens, capturing ids
    // so the rendered rows can be targeted by stable `data-memory-id`.
    const matchText = 'User prefers the ZEBRACORN editor for everything'
    const otherText = 'User commutes by WALRUSBOAT on weekdays'
    const ids: Record<string, string> = {}
    for (const content of [matchText, otherText]) {
      const res = await page.request.post(`${apiURL}/api/memories`, {
        headers: authHeader,
        data: { content, kind: 'fact' },
      })
      expect(res.status()).toBe(201)
      ids[content] = (await res.json()).id as string
    }

    await page.goto(`${baseURL}/settings/memory`)

    // Both memories render before filtering.
    await expect(page.locator(`[data-memory-id="${ids[matchText]}"]`)).toBeVisible({ timeout: 10_000 })
    await expect(page.locator(`[data-memory-id="${ids[otherText]}"]`)).toBeVisible()

    // Search for a token unique to the first memory. The store fires a
    // debounced GET /api/memories?search=ZEBRACORN — assert it goes to
    // the server with the query (proves the real filtered fetch, not a
    // client-only filter).
    const searchReq = page.waitForRequest(
      req =>
        req.url().includes('/api/memories') &&
        /[?&]search=ZEBRACORN/.test(decodeURIComponent(req.url())),
    )
    await byTestId(page, 'memory-search-input').fill('ZEBRACORN')
    await searchReq

    // The matching memory stays; the non-matching one is filtered out.
    await expect(page.locator(`[data-memory-id="${ids[matchText]}"]`)).toBeVisible({ timeout: 10_000 })
    await expect(page.locator(`[data-memory-id="${ids[otherText]}"]`)).toHaveCount(0)
  })
})
