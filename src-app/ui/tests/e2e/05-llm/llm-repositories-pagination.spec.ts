import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'

/**
 * E2E — LlmRepositorySettings pagination (the Pagination control at
 * `LlmRepositorySettings.tsx:328-341` was never driven through the UI).
 *
 * Seeds enough repositories to span multiple pages, then drives the antd
 * Pagination: asserts the "X-Y of N repositories" total, shrinks the page size
 * via the size changer, and pages forward — asserting the visible range updates.
 */

test.describe('LLM Repositories — pagination', () => {
  test('page size + page navigation update the visible range', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)

    // Seed 12 repositories (auth_type none) so the list spans >1 page.
    const ts = Date.now().toString(36)
    for (let i = 0; i < 12; i++) {
      const res = await page.request.post(`${apiURL}/api/llm-repositories`, {
        headers: { Authorization: `Bearer ${token}` },
        data: {
          name: `PageRepo-${ts}-${String(i).padStart(2, '0')}`,
          url: `https://example.com/repo-${ts}-${i}`,
          auth_type: 'none',
          enabled: true,
        },
      })
      expect(res.status()).toBe(201)
    }

    await page.goto(`${baseURL}/settings/llm-repositories`)

    // The Pagination total renders "X-Y of N repositories".
    const total = page.getByText(/\d+-\d+ of \d+ repositories/)
    await expect(total).toBeVisible({ timeout: 30000 })

    // Shrink the page size to 5 via the size changer.
    await page.locator('.ant-pagination-options .ant-select').click()
    await page
      .locator('.ant-select-dropdown:not(.ant-select-dropdown-hidden)')
      .getByText('5 / page')
      .click()

    // Now the first page shows items 1-5.
    await expect(page.getByText(/^1-5 of \d+ repositories$/)).toBeVisible({
      timeout: 10000,
    })

    // Page forward → the range advances to 6-10.
    await page.locator('.ant-pagination-item-2').click()
    await expect(page.getByText(/^6-10 of \d+ repositories$/)).toBeVisible({
      timeout: 10000,
    })
  })
})
