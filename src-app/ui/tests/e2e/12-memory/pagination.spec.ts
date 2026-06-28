import { test, expect } from '../../fixtures/test-context'
import {
  getAdminToken,
  createTestUser,
  login,
  getCurrentUserToken,
} from '../../common/auth-helpers'

/**
 * E2E — MyMemoriesSection pagination on /settings/memory.
 *
 * The section paginates at pageSize=10 (antd Pagination with a "X-Y of N
 * memories" total + page-size options). With >10 memories seeded, the first
 * page shows 10, the total reads "of N", and navigating to page 2 reveals the
 * remaining rows (a memory that was NOT on page 1). Existing memory specs cover
 * add/list/delete/edit/filter but never the pagination control.
 */

async function memoryUser(apiURL: string, name: string) {
  const adminToken = await getAdminToken(apiURL)
  const username = `${name}_${Date.now().toString(36)}`
  await createTestUser(apiURL, adminToken, username, `${username}@ex.com`, 'password123', [
    'profile::read',
    'profile::edit',
    'memory::read',
    'memory::write',
  ])
  return username
}

test.describe('Memory — pagination', () => {
  test('paginates the memory list past the first page', async ({ page, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    const username = await memoryUser(apiURL, 'mem_page')
    await login(page, baseURL, username, 'password123')
    const token = await getCurrentUserToken(page)

    // Seed 12 memories (> pageSize=10) → exactly 2 pages. Distinctive,
    // zero-padded content so a page-2 row is uniquely assertable.
    const total = 12
    for (let i = 0; i < total; i++) {
      const res = await page.request.post(`${apiURL}/api/memories`, {
        headers: { Authorization: `Bearer ${token}` },
        data: {
          content: `PAGEMEM_${String(i).padStart(2, '0')} distinct fact number ${i}`,
          kind: 'fact',
        },
      })
      expect(res.status()).toBe(201)
    }

    await page.goto(`${baseURL}/settings/memory`)

    // The pagination total reflects all 12 (showTotal: "1-10 of 12 memories").
    await expect(page.getByText(/1-10 of 12 memories/)).toBeVisible({ timeout: 15000 })

    // Page 1 shows 10 rows; the antd Pagination has a "2" page button.
    const rows = page.locator('[data-memory-id]')
    await expect(rows).toHaveCount(10)
    const page2 = page.locator('.ant-pagination .ant-pagination-item-2')
    await expect(page2).toBeVisible()

    // Go to page 2 → the remaining 2 rows; total label updates to "11-12".
    await page2.click()
    await expect(page.getByText(/11-12 of 12 memories/)).toBeVisible()
    await expect(rows).toHaveCount(2)
    // A row that lives only on page 2 (one of the last two by created_at order)
    // is now visible — proving the page actually advanced.
    await expect(
      page.locator('[data-memory-id]').filter({ hasText: /PAGEMEM_0[01] / }),
    ).toHaveCount(2)
  })
})
