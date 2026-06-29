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

    await byTestId(page, 'memory-add-btn').click()
    const dialog = page.getByRole('dialog')
    await expect(dialog).toBeVisible()

    // Submit with empty Content → inline required validation, no memory row created.
    await byTestId(page, 'memory-create-submit-btn').click()
    await expect(byTestId(dialog, 'field-error-content')).toBeVisible({
      timeout: 5000,
    })
    await expect(page.locator('[data-memory-id]')).toHaveCount(0)
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
    const id = await row.getAttribute('data-memory-id')
    await byTestId(row, `memory-row-edit-btn-${id}`).click()
    const dialog = page.getByRole('dialog')
    await expect(dialog).toBeVisible()
    await expect(byTestId(dialog, 'memory-edit-content-input')).toHaveValue(
      'Original memory content ABC',
    )

    // Edit + save → the list shows the new content.
    await byTestId(dialog, 'memory-edit-content-input').fill(
      'Edited memory content XYZ',
    )
    await byTestId(page, 'memory-edit-submit-btn').click()
    await expect(
      page.locator('[data-memory-id]').filter({ hasText: 'Edited memory content XYZ' }),
    ).toBeVisible({ timeout: 5000 })
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
    await expect(byTestId(page, 'memory-pagination')).toBeVisible({
      timeout: 15000,
    })

    // Shrink page size to 5 → first page shows exactly 5 rows.
    await byTestId(page, 'memory-pagination-page-size').click()
    await byTestId(page, 'memory-pagination-page-size-opt-5').click()
    await expect(page.locator('[data-memory-id]')).toHaveCount(5, {
      timeout: 10000,
    })

    // Page forward → page 2 still shows 5 rows (12 total → 5/5/2).
    await byTestId(page, 'memory-pagination-page-2').click()
    await expect(page.locator('[data-memory-id]')).toHaveCount(5, {
      timeout: 10000,
    })
  })
})
