import { test, expect } from '../../fixtures/test-context'
import {
  loginAsAdmin,
  getAdminToken,
  getCurrentUserToken,
  createTestUser,
  login,
} from '../../common/auth-helpers'

/**
 * E2E — MyMemoriesSection UI flows the manual-add spec doesn't cover:
 *   - CreateMemoryDrawer validation (empty Content → "Required")
 *   - EditMemoryDrawer (open pre-seeded → edit → save)
 *   - Pagination page-size change + page navigation
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

test.describe('Memory — CRUD UI', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    await loginAsAdmin(page, testInfra.baseURL)
  })

  test('CreateMemoryDrawer rejects empty content with a required error', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    const username = await memoryUser(apiURL, 'mem_validate')
    await login(page, baseURL, username, 'password123')
    await page.goto(`${baseURL}/settings/memory`)

    await page.getByRole('button', { name: /Add memory/ }).click()
    const dialog = page.getByRole('dialog')
    await expect(dialog).toBeVisible()

    // Submit with empty Content → inline "Required" validation, no success toast.
    await dialog.getByRole('button', { name: /^Add$/ }).click()
    await expect(dialog.getByText('Required')).toBeVisible({ timeout: 5000 })
    await expect(page.getByText('Memory added')).toHaveCount(0)
  })

  test('EditMemoryDrawer opens pre-seeded and saves an edit', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    const username = await memoryUser(apiURL, 'mem_edit')
    await login(page, baseURL, username, 'password123')
    const token = await getCurrentUserToken(page)
    // Seed a memory via REST.
    await page.request.post(`${apiURL}/api/memories`, {
      headers: { Authorization: `Bearer ${token}` },
      data: { content: 'Original memory content ABC' },
    })

    await page.goto(`${baseURL}/settings/memory`)
    const row = page.locator('[data-memory-id]').filter({ hasText: 'ABC' })
    await expect(row).toBeVisible({ timeout: 15000 })

    // Open the row's Edit drawer; it must be pre-seeded with the row content.
    await row.getByRole('button', { name: 'Edit memory' }).click()
    const dialog = page.getByRole('dialog')
    await expect(dialog).toBeVisible()
    await expect(dialog.getByLabel('Content')).toHaveValue(
      'Original memory content ABC',
    )

    // Edit + save → success + the list shows the new content.
    await dialog.getByLabel('Content').fill('Edited memory content XYZ')
    await dialog.getByRole('button', { name: /^Save$/ }).click()
    await expect(page.getByText('Memory updated')).toBeVisible({ timeout: 5000 })
    await expect(page.getByText('Edited memory content XYZ')).toBeVisible()
  })

  test('pagination page-size change + page navigation', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    const username = await memoryUser(apiURL, 'mem_page')
    await login(page, baseURL, username, 'password123')
    const token = await getCurrentUserToken(page)
    // Seed 12 memories so the list spans more than one page.
    for (let i = 0; i < 12; i++) {
      await page.request.post(`${apiURL}/api/memories`, {
        headers: { Authorization: `Bearer ${token}` },
        data: { content: `Paged memory number ${String(i).padStart(2, '0')}` },
      })
    }

    await page.goto(`${baseURL}/settings/memory`)
    // The pagination total renders "X-Y of N memories".
    await expect(page.getByText(/\d+-\d+ of \d+ memories/)).toBeVisible({
      timeout: 15000,
    })

    // Shrink page size to 5 → "1-5 of N".
    await page.locator('.ant-pagination-options .ant-select').click()
    await page
      .locator('.ant-select-dropdown:not(.ant-select-dropdown-hidden)')
      .getByText('5 / page')
      .click()
    await expect(page.getByText(/^1-5 of \d+ memories$/)).toBeVisible({
      timeout: 10000,
    })

    // Page forward → "6-10 of N".
    await page.locator('.ant-pagination-item-2').click()
    await expect(page.getByText(/^6-10 of \d+ memories$/)).toBeVisible({
      timeout: 10000,
    })
  })
})
