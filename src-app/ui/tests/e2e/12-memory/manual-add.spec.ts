import { test, expect } from '../../fixtures/test-context'
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
    // After the settings-page consolidation, "My memories" is a Card
    // section title (not a Typography heading), so anchor on the
    // section's unique CTA instead.
    await expect(
      page.getByRole('button', { name: /Add memory/ }),
    ).toBeVisible()

    // Add
    await page.getByRole('button', { name: /Add memory/ }).click()
    const dialog = page.getByRole('dialog')
    await expect(dialog).toBeVisible()
    await dialog.getByLabel('Content').fill('User prefers TypeScript over JavaScript')
    await dialog.getByRole('button', { name: /^Add$/ }).click()
    await expect(page.getByText('Memory added')).toBeVisible({ timeout: 5000 })

    // List row appears
    await expect(
      page.getByText('User prefers TypeScript over JavaScript'),
    ).toBeVisible()

    // Delete — memories render as divs with `data-memory-id`, NOT as
    // table rows. Find the memory's wrapper by content, then click the
    // delete button by its aria-label prefix. After clicking, the
    // Popconfirm opens AND the trash icon's tooltip "Delete memory"
    // also lingers (it's a portaled overlay at body-level, so scoping
    // to the Popconfirm doesn't escape it). `force: true` bypasses the
    // pointer-intercept check on the OK button — standard antd
    // tooltip-over-popconfirm workaround.
    await page
      .locator('[data-memory-id]')
      .filter({ hasText: 'TypeScript' })
      .getByRole('button', { name: /Delete memory/ })
      .click()
    // dispatchEvent('click') sends a synthetic click directly to the
    // button regardless of viewport / overlays — `force: true` needs
    // the element in viewport, and `click()` blocks on the tooltip
    // intercept. The Popconfirm's onConfirm fires identically.
    await page
      .locator('.ant-popconfirm')
      .getByRole('button', { name: 'Delete', exact: true })
      .dispatchEvent('click')
    await expect(page.getByText('Memory deleted')).toBeVisible({ timeout: 5000 })
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
    await expect(page.getByText(original)).toBeVisible({ timeout: 15000 })

    // Open the row's Edit drawer, change the Content, save.
    await page
      .locator('[data-memory-id]')
      .filter({ hasText: original })
      .getByRole('button', { name: 'Edit memory' })
      .click()
    const drawer = page.getByRole('dialog', { name: 'Edit memory' })
    await expect(drawer).toBeVisible()
    const updated = `${original}_UPDATED`
    await drawer.getByLabel('Content').fill(updated)
    await drawer.getByRole('button', { name: 'Save' }).click()

    await expect(page.getByText('Memory updated')).toBeVisible({ timeout: 5000 })
    // The list reflects the edited content (and no longer the original).
    await expect(page.getByText(updated)).toBeVisible()
    await expect(page.getByText(original, { exact: true })).toHaveCount(0)
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
    await expect(page.getByText('Exportable memory row')).toBeVisible({
      timeout: 15000,
    })

    // Export as JSON → a ziee-memories-*.json download.
    let download = page.waitForEvent('download')
    await page.getByRole('button', { name: 'Export' }).click()
    await page.getByText('Export as JSON').click()
    const jsonFile = await download
    expect(jsonFile.suggestedFilename()).toMatch(/^ziee-memories-.*\.json$/)

    // Export as CSV → a ziee-memories-*.csv download.
    download = page.waitForEvent('download')
    await page.getByRole('button', { name: 'Export' }).click()
    await page.getByText('Export as CSV').click()
    const csvFile = await download
    expect(csvFile.suggestedFilename()).toMatch(/^ziee-memories-.*\.csv$/)
  })
})
