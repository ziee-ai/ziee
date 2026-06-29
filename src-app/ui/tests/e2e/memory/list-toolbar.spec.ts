import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'
import {
  loginAsAdmin,
  getAdminToken,
  getCurrentUserToken,
  createTestUser,
  login,
} from '../../common/auth-helpers'

/**
 * E2E — MyMemoriesSection list TOOLBAR + PAGINATION on /settings/memory.
 *
 * manual-add.spec.ts covers add/list/delete but never exercised the filter
 * toolbar (search / export / delete-all) or the pagination size changer
 * (MyMemoriesSection.tsx:131-189, 304-316). Memories are seeded via the REST
 * surface (faster than the Add dialog); the toolbar + pagination run through
 * the real UI.
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

async function seedMemories(page: import('@playwright/test').Page, apiURL: string, contents: string[]) {
  const token = await getCurrentUserToken(page)
  for (const content of contents) {
    const res = await page.request.post(`${apiURL}/api/memories`, {
      headers: { Authorization: `Bearer ${token}` },
      data: { content },
    })
    expect(res.ok()).toBeTruthy()
  }
}

test.describe('Memory — list toolbar + pagination', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    await loginAsAdmin(page, testInfra.baseURL)
  })

  test('pagination size changer switches page size', async ({ page, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    const username = await memoryUser(apiURL, 'mem_page')
    await login(page, baseURL, username, 'password123')

    // 12 memories > the default page size of 10 → a second page exists.
    await seedMemories(
      page,
      apiURL,
      Array.from({ length: 12 }, (_, i) => `Pagination memory number ${String(i).padStart(2, '0')}`),
    )

    await page.goto(`${baseURL}/settings/memory`)
    // Default page shows 10 of 12 rows.
    await expect(page.locator('[data-memory-id]')).toHaveCount(10, { timeout: 30000 })

    // Bump the page size to 20 via the size changer → all 12 on one page.
    await byTestId(page, 'memory-pagination-page-size').click()
    await byTestId(page, 'memory-pagination-page-size-opt-20').click()
    await expect(page.locator('[data-memory-id]')).toHaveCount(12, { timeout: 10000 })
  })

  test('search filter narrows the list; export downloads; delete-all clears', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    const username = await memoryUser(apiURL, 'mem_filter')
    await login(page, baseURL, username, 'password123')

    await seedMemories(page, apiURL, [
      'ALPHA distinctive marker for the search filter',
      'BETA distinctive marker for the search filter',
    ])

    await page.goto(`${baseURL}/settings/memory`)
    const alpha = page
      .locator('[data-memory-id]')
      .filter({ hasText: 'ALPHA distinctive marker for the search filter' })
    const beta = page
      .locator('[data-memory-id]')
      .filter({ hasText: 'BETA distinctive marker for the search filter' })
    await expect(alpha).toBeVisible({ timeout: 30000 })

    // Search narrows to just the ALPHA row.
    await byTestId(page, 'memory-search-input').fill('ALPHA')
    await expect(alpha).toBeVisible({ timeout: 10000 })
    await expect(beta).toHaveCount(0, { timeout: 10000 })
    // Clear the search → BETA returns.
    await byTestId(page, 'memory-search-input').fill('')
    await expect(beta).toBeVisible({ timeout: 10000 })

    // Export → the JSON download fires.
    const downloadPromise = page.waitForEvent('download')
    await byTestId(page, 'memory-export-btn').click()
    await byTestId(page, 'memory-export-dropdown-item-json').click()
    const download = await downloadPromise
    expect(download.suggestedFilename()).toMatch(/\.json$/)

    // Delete all → confirm dialog → confirm → list empties.
    await byTestId(page, 'memory-delete-all-btn').click()
    await byTestId(page, 'memory-delete-all-confirm-confirm').click()
    await expect(page.locator('[data-memory-id]')).toHaveCount(0, { timeout: 10000 })
  })
})
