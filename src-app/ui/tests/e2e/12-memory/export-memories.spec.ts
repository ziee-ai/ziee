import { test, expect } from '../../fixtures/test-context'
import {
  loginAsAdmin,
  getAdminToken,
  createTestUser,
  login,
} from '../../common/auth-helpers'

/**
 * E2E — MyMemoriesSection export (JSON / CSV) on /settings/memory.
 * audit id 0af90cbb63e4 — the Export dropdown (exportMemories(), JSON + CSV)
 * had no E2E coverage. Adds a memory through the UI, then drives the real
 * Export menu and asserts a download fires with the right filename + content.
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

const MEMORY_TEXT = 'User prefers Rust for systems programming'

test.describe('Memory — export', () => {
  test.describe.configure({ retries: 2 })

  test.beforeEach(async ({ page, testInfra }) => {
    await loginAsAdmin(page, testInfra.baseURL)
  })

  test('exports memories as JSON and CSV', async ({ page, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    const username = await memoryUser(apiURL, 'mem_export')
    await login(page, baseURL, username, 'password123')

    await page.goto(`${baseURL}/settings/memory`)
    await expect(page.getByRole('button', { name: /Add memory/ })).toBeVisible()

    // Seed one memory through the UI so the export has real content.
    await page.getByRole('button', { name: /Add memory/ }).click()
    const dialog = page.getByRole('dialog')
    await expect(dialog).toBeVisible()
    await dialog.getByLabel('Content').fill(MEMORY_TEXT)
    await dialog.getByRole('button', { name: /^Add$/ }).click()
    await expect(page.getByText('Memory added')).toBeVisible({ timeout: 5000 })
    await expect(page.getByText(MEMORY_TEXT)).toBeVisible()

    // Export as JSON.
    const jsonDownloadPromise = page.waitForEvent('download')
    await page.getByRole('button', { name: 'Export' }).click()
    await page.getByText('Export as JSON').click()
    const jsonDownload = await jsonDownloadPromise
    expect(jsonDownload.suggestedFilename()).toMatch(/^ziee-memories-\d{4}-\d{2}-\d{2}\.json$/)
    const jsonPath = await jsonDownload.path()
    const { readFileSync } = await import('fs')
    expect(readFileSync(jsonPath, 'utf8')).toContain(MEMORY_TEXT)

    // Export as CSV.
    const csvDownloadPromise = page.waitForEvent('download')
    await page.getByRole('button', { name: 'Export' }).click()
    await page.getByText('Export as CSV').click()
    const csvDownload = await csvDownloadPromise
    expect(csvDownload.suggestedFilename()).toMatch(/^ziee-memories-\d{4}-\d{2}-\d{2}\.csv$/)
    const csvText = readFileSync(await csvDownload.path(), 'utf8')
    // The CSV header + the memory content row are present.
    expect(csvText).toContain('id,content,kind,source')
    expect(csvText).toContain(MEMORY_TEXT)
  })
})
