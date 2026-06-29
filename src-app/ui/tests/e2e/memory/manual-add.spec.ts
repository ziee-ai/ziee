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
 * E2E — manual memory add + list + delete on the /settings/memory page.
 *
 * Phase 1 plan §9: "MemoriesPage.tsx with manual add/list/edit/delete
 * (no embedding, no AI; pure text storage)". This spec exercises that
 * happy path against the live REST surface.
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

test.describe('Memory — manual add', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    await loginAsAdmin(page, testInfra.baseURL)
  })

  test('add → list → delete', async ({ page, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    const username = await memoryUser(apiURL, 'mem_add')
    await login(page, baseURL, username, 'password123')

    await page.goto(`${baseURL}/settings/memory`)
    // Anchor on the section's unique CTA.
    await expect(byTestId(page, 'memory-add-btn')).toBeVisible()

    // Add
    await byTestId(page, 'memory-add-btn').click()
    const dialog = page.getByRole('dialog')
    await expect(dialog).toBeVisible()
    await byTestId(dialog, 'memory-create-content-input').fill(
      'User prefers TypeScript over JavaScript',
    )
    await byTestId(page, 'memory-create-submit-btn').click()

    // List row appears (content is dynamic data the test typed).
    const row = page
      .locator('[data-memory-id]')
      .filter({ hasText: 'User prefers TypeScript over JavaScript' })
    await expect(row).toBeVisible({ timeout: 5000 })

    // Delete — memories render as divs with `data-memory-id`; the per-row
    // delete button + its confirm carry derived testids keyed on the id.
    const id = await row.getAttribute('data-memory-id')
    await byTestId(row, `memory-row-delete-btn-${id}`).click()
    await byTestId(page, `memory-row-delete-confirm-${id}-confirm`).click()
    await expect(row).toHaveCount(0, { timeout: 5000 })
  })

  test('edit a memory via the Edit drawer', async ({ page, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    const username = await memoryUser(apiURL, 'mem_edit')
    await login(page, baseURL, username, 'password123')

    // Seed a memory via the user's own REST token.
    const token = await getCurrentUserToken(page)
    const original = `EDITME_${Date.now().toString(36)}`
    const created = await page.request.post(`${apiURL}/api/memories`, {
      headers: { Authorization: `Bearer ${token}` },
      data: { content: original, kind: 'fact' },
    })
    expect(created.status()).toBe(201)

    await page.goto(`${baseURL}/settings/memory`)
    const row = page.locator('[data-memory-id]').filter({ hasText: original })
    await expect(row).toBeVisible({ timeout: 15000 })

    // Open the row's Edit drawer, change the Content, save.
    const id = await row.getAttribute('data-memory-id')
    await byTestId(row, `memory-row-edit-btn-${id}`).click()
    const drawer = page.getByRole('dialog')
    await expect(byTestId(drawer, 'memory-edit-form')).toBeVisible()
    const updated = `${original}_UPDATED`
    await byTestId(drawer, 'memory-edit-content-input').fill(updated)
    await byTestId(page, 'memory-edit-submit-btn').click()

    // The list reflects the edited content (and no longer the original).
    await expect(
      page.locator('[data-memory-id]').filter({ hasText: updated }),
    ).toBeVisible({ timeout: 5000 })
    await expect(
      page.locator('[data-memory-id]').filter({ hasText: original }),
    ).toHaveCount(0)
  })

  test('exports memories as JSON and CSV', async ({ page, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    const username = await memoryUser(apiURL, 'mem_export')
    await login(page, baseURL, username, 'password123')
    const token = await getCurrentUserToken(page)
    const created = await page.request.post(`${apiURL}/api/memories`, {
      headers: { Authorization: `Bearer ${token}` },
      data: { content: 'Exportable memory row', kind: 'fact' },
    })
    expect(created.status()).toBe(201)

    await page.goto(`${baseURL}/settings/memory`)
    await expect(
      page.locator('[data-memory-id]').filter({ hasText: 'Exportable memory row' }),
    ).toBeVisible({ timeout: 15000 })

    // Export as JSON → a ziee-memories-*.json download.
    let download = page.waitForEvent('download')
    await byTestId(page, 'memory-export-btn').click()
    await byTestId(page, 'memory-export-dropdown-item-json').click()
    const jsonFile = await download
    expect(jsonFile.suggestedFilename()).toMatch(/^ziee-memories-.*\.json$/)

    // Export as CSV → a ziee-memories-*.csv download.
    download = page.waitForEvent('download')
    await byTestId(page, 'memory-export-btn').click()
    await byTestId(page, 'memory-export-dropdown-item-csv').click()
    const csvFile = await download
    expect(csvFile.suggestedFilename()).toMatch(/^ziee-memories-.*\.csv$/)
  })
})
