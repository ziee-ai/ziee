import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'
import {
  getAdminToken,
  createTestUser,
  login,
  getCurrentUserToken,
} from '../../common/auth-helpers'

/**
 * MyMemoriesSection toolbar actions on /settings/memory not covered by the
 * kind-filter spec: the content SEARCH box, the SOURCE filter, and the
 * "Delete all" bulk action. Seeds via REST, then drives each control and
 * asserts the list reacts.
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

test.describe('Memory — filter toolbar', () => {
  test('search filters by content; source filter + delete-all work', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    const username = await memoryUser(apiURL, 'mem_toolbar')
    await login(page, baseURL, username, 'password123')
    const token = await getCurrentUserToken(page)

    const seed = async (content: string, kind: string) => {
      const r = await page.request.post(`${apiURL}/api/memories`, {
        headers: { Authorization: `Bearer ${token}` },
        data: { content, kind },
      })
      expect(r.status()).toBe(201)
    }
    await seed('ZEBRA loves typescript', 'preference')
    await seed('GIRAFFE prefers rust', 'preference')
    await seed('PENGUIN enjoys python', 'fact')

    await page.goto(`${baseURL}/settings/memory`)
    const rows = page.locator('[data-memory-id]')
    await expect(rows).toHaveCount(3, { timeout: 15000 })

    // SEARCH: typing a unique token narrows the list to the matching memory.
    await byTestId(page, 'memory-search-input').fill('GIRAFFE')
    await expect(rows).toHaveCount(1)
    await expect(rows.filter({ hasText: 'GIRAFFE prefers rust' })).toBeVisible()
    // Clear the search → all three return.
    await byTestId(page, 'memory-search-input').fill('')
    await expect(rows).toHaveCount(3)

    // SOURCE: the REST-seeded memories are 'manual'; filtering to
    // 'Auto-extracted' yields none, and clearing restores them.
    await byTestId(page, 'memory-source-filter').click()
    await byTestId(page, 'memory-source-filter-opt-extraction').click()
    await expect(rows).toHaveCount(0)
    // Clear the Source select (the allow-clear X).
    await byTestId(page, 'memory-source-filter-clear').click()
    await expect(rows).toHaveCount(3)

    // DELETE ALL: the danger confirm wipes the library.
    await byTestId(page, 'memory-delete-all-btn').click()
    await byTestId(page, 'memory-delete-all-confirm-confirm').click()
    await expect(rows).toHaveCount(0, { timeout: 10000 })
  })
})
