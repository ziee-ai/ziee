import { test, expect } from '../../fixtures/test-context'
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
    await page.getByPlaceholder('Search content').fill('GIRAFFE')
    await expect(rows).toHaveCount(1)
    await expect(page.getByText('GIRAFFE prefers rust')).toBeVisible()
    // Clear the search → all three return.
    await page.getByPlaceholder('Search content').clear()
    await expect(rows).toHaveCount(3)

    // SOURCE: the REST-seeded memories are 'manual'; filtering to
    // 'Auto-extracted' yields none, and clearing restores them.
    await page.getByText('Source', { exact: true }).click()
    await page.getByRole('option', { name: 'Auto-extracted' }).click()
    await expect(rows).toHaveCount(0)
    // Clear the Source select (the allow-clear X).
    await page.locator('.ant-select-clear').first().click()
    await expect(rows).toHaveCount(3)

    // DELETE ALL: the danger Popconfirm wipes the library.
    await page.getByRole('button', { name: 'Delete all' }).click()
    await page
      .locator('.ant-popconfirm')
      .getByRole('button', { name: 'Delete', exact: true })
      .click()
    await expect(rows).toHaveCount(0, { timeout: 10000 })
  })
})
