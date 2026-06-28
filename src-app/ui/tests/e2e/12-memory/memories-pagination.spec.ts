import { test, expect } from '../../fixtures/test-context'
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
 * MyMemoriesSection.tsx:300-318 renders an antd `<Pagination>` (default
 * `pageSize: 10`, Memories.store.ts:116) whose `onChange` drives
 * `Stores.Memories.load(page, size)` → a SERVER-side page fetch
 * (`GET /api/memories?page=<n>&per_page=<size>`, store lines 60-87). No prior
 * 12-memory spec drove the pagination control (search/filter specs seed only
 * two rows). This seeds 12 memories so the list spills onto a second page,
 * then clicks to page 2 and asserts the real `?page=2` fetch fires and the
 * `showTotal` range advances (1-10 → 11-12), proving server-side paging.
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
  test('clicking page 2 fetches the next server page and advances the range', async ({
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

    // Page 1: the pagination control reports the first page's range out of 12.
    await expect(
      page.getByText(`1-10 of ${TOTAL} memories`),
    ).toBeVisible({ timeout: 10_000 })
    // Exactly 10 of the 12 rows render on page 1 (each row is one
    // `<div data-memory-id>` in MyMemoriesSection.tsx:204).
    await expect(page.locator('[data-memory-id]')).toHaveCount(10)

    // Click page 2 — antd renders each page item as a list item titled by its
    // number. The store fires a real GET /api/memories?page=2 server fetch.
    const pageTwoReq = page.waitForRequest(
      req =>
        req.url().includes('/api/memories') &&
        /[?&]page=2(?:&|$)/.test(decodeURIComponent(req.url())),
    )
    await page.locator('.ant-pagination-item[title="2"]').click()
    await pageTwoReq

    // Page 2: the range advances to the final two memories.
    await expect(
      page.getByText(`11-${TOTAL} of ${TOTAL} memories`),
    ).toBeVisible({ timeout: 10_000 })
    // Only the remaining 2 rows render on page 2 (the list content changed).
    await expect(page.locator('[data-memory-id]')).toHaveCount(2)
  })
})
