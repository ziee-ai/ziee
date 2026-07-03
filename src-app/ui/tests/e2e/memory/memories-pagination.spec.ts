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
 * E2E — MyMemoriesSection pagination (audit gap all-e418d4fb51f9).
 *
 * MyMemoriesSection renders a kit `<Pagination>` (default `pageSize: 10`)
 * whose `onChange` drives `Stores.Memories.load(page, size)` → a SERVER-side
 * page fetch (`GET /api/memories?page=<n>&per_page=<size>`). No prior
 * memory spec drove the pagination control (search/filter specs seed only
 * two rows). This seeds 12 memories so the list spills onto a second page,
 * jumps to page 2 via the quick-jumper, and asserts the real `?page=2` fetch
 * fires and the rendered row set changes (10 → 2), proving server-side paging.
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

test.describe('Memory — My memories pagination', () => {
  test('jumping to page 2 fetches the next server page and changes the rows', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const username = await memoryUser(apiURL, 'mem_page')
    await login(page, baseURL, username, 'password123')
    const authHeader = {
      Authorization: `Bearer ${await getCurrentUserToken(page)}`,
    }

    // Seed 12 memories — exceeds the default page size of 10, so the list
    // spans two pages (10 + 2). Each carries a unique numbered beacon.
    const TOTAL = 12
    for (let i = 0; i < TOTAL; i++) {
      const n = String(i).padStart(2, '0')
      const res = await page.request.post(`${apiURL}/api/memories`, {
        headers: authHeader,
        data: { content: `PAGEBEACON_${n} memory fact number ${n}`, kind: 'fact' },
      })
      expect(res.status()).toBe(201)
    }

    await page.goto(`${baseURL}/settings/memory`)

    // Page 1: the pagination control is present and exactly 10 of the 12 rows
    // render (each row is one `<div data-memory-id>`).
    await expect(byTestId(page, 'memory-pagination')).toBeVisible({ timeout: 10_000 })
    await expect(page.locator('[data-memory-id]')).toHaveCount(10)

    // Jump to page 2 via the quick-jumper input. The store fires a real
    // GET /api/memories?page=2 server fetch. (ListPagination renders the
    // shared kit quick-jumper — `${testid}-jump` — on every list.)
    const pageTwoReq = page.waitForRequest(
      req =>
        req.url().includes('/api/memories') &&
        /[?&]page=2(?:&|$)/.test(decodeURIComponent(req.url())),
    )
    await byTestId(page, 'memory-pagination-jump').fill('2')
    await byTestId(page, 'memory-pagination-jump').press('Enter')
    await pageTwoReq

    // Page 2: only the remaining 2 rows render (the list content changed).
    await expect(page.locator('[data-memory-id]')).toHaveCount(2, { timeout: 10_000 })
  })
})
