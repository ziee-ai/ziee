import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'

/**
 * MyMemoriesSection kind/source filters (/settings/memory). Each filter setter
 * re-queries the backend (Stores.Memories.setKindFilter -> loadMemories), so
 * this seeds memories of different kinds via the REST API and asserts the list
 * re-filters when a Kind is chosen.
 */
test.describe('Memory — my-memories kind/source filter', () => {
  test('selecting a Kind filters the memory list', async ({ page, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    const auth = { Authorization: `Bearer ${token}` }

    const ts = Date.now()
    const factText = `FACTMEM_${ts}`
    const prefText = `PREFMEM_${ts}`
    for (const [content, kind] of [
      [factText, 'fact'],
      [prefText, 'preference'],
    ]) {
      const res = await page.request.post(`${apiURL}/api/memories`, {
        headers: auth,
        data: { content, kind },
      })
      expect(res.status()).toBe(201)
    }

    await page.goto(`${baseURL}/settings/memory`)
    const card = page
      .locator('.ant-card')
      .filter({ hasText: 'Search content' })
    await expect(card).toBeVisible({ timeout: 20000 })

    // Both memories show with no filter.
    await expect(page.getByText(factText)).toBeVisible({ timeout: 15000 })
    await expect(page.getByText(prefText)).toBeVisible()

    // Filter by Kind = Fact → only the fact memory remains (backend re-query).
    // The Kind Select is identified by its "Kind" placeholder.
    const kindSelect = card
      .locator('.ant-select')
      .filter({ has: page.getByText('Kind', { exact: true }) })
    await kindSelect.click()
    await page.getByRole('option', { name: 'Fact', exact: true }).click()

    await expect(page.getByText(factText)).toBeVisible()
    await expect(page.getByText(prefText)).toHaveCount(0)
  })
})
