import { test, expect } from '../../fixtures/test-context'
import {
  loginAsAdmin,
  getAdminToken,
  createTestUser,
  login,
  getCurrentUserToken,
} from '../../common/auth-helpers'

/**
 * E2E — MyMemoriesSection export dropdown (audit gap e4ddd68f3791).
 *
 * MyMemoriesSection.tsx:87-101 renders an antd `<Dropdown>` "Export"
 * button with two menu items — "Export as JSON" and "Export as CSV" —
 * each calling the client-side `exportMemories(memories, fmt)` helper
 * (MyMemoriesSection.tsx:334) which builds a Blob and triggers a
 * `ziee-memories-<date>.{json,csv}` download. No prior spec drove that
 * dropdown; this seeds a memory, exports each format through the real
 * UI, and asserts the downloaded bytes are correctly shaped (parseable
 * JSON containing the memory; CSV with the documented header row + the
 * memory content), proving the real export output — nothing mocked.
 */

async function memoryUser(apiURL: string, name: string) {
  const adminToken = await getAdminToken(apiURL)
  const username = `${name}_${Date.now().toString(36)}`
  await createTestUser(
    apiURL,
    adminToken,
    username,
    `${username}@ex.com`,
    'password123',
    ['profile::read', 'profile::edit', 'memory::read', 'memory::write'],
  )
  return username
}

async function downloadText(
  page: import('@playwright/test').Page,
  trigger: () => Promise<void>,
): Promise<{ filename: string; body: string }> {
  const [download] = await Promise.all([
    page.waitForEvent('download'),
    trigger(),
  ])
  const stream = await download.createReadStream()
  const chunks: Buffer[] = []
  for await (const chunk of stream) chunks.push(chunk as Buffer)
  return {
    filename: download.suggestedFilename(),
    body: Buffer.concat(chunks).toString('utf-8'),
  }
}

test.describe('Memory — My memories export dropdown', () => {
  test('Export as JSON and CSV produce correctly-shaped downloads', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const username = await memoryUser(apiURL, 'mem_export')
    await login(page, baseURL, username, 'password123')
    const authHeader = {
      Authorization: `Bearer ${await getCurrentUserToken(page)}`,
    }

    // Seed a memory with a unique content token + a comma so the CSV
    // RFC-4180 quoting path is exercised too.
    const content = 'User prefers the QUOKKADECK editor, especially on Fridays'
    const res = await page.request.post(`${apiURL}/api/memories`, {
      headers: authHeader,
      data: { content, kind: 'fact' },
    })
    expect(res.status()).toBe(201)

    await page.goto(`${baseURL}/settings/memory`)
    await expect(page.getByText(content)).toBeVisible({ timeout: 10_000 })

    // --- Export as JSON ---
    const json = await downloadText(page, async () => {
      await page.getByRole('button', { name: 'Export' }).click()
      await page.getByRole('menuitem', { name: 'Export as JSON' }).click()
    })
    expect(json.filename).toMatch(/^ziee-memories-\d{4}-\d{2}-\d{2}\.json$/)
    const parsed = JSON.parse(json.body) as Array<{ content: string }>
    expect(Array.isArray(parsed)).toBe(true)
    expect(parsed.some((m) => m.content === content)).toBe(true)

    // --- Export as CSV ---
    const csv = await downloadText(page, async () => {
      await page.getByRole('button', { name: 'Export' }).click()
      await page.getByRole('menuitem', { name: 'Export as CSV' }).click()
    })
    expect(csv.filename).toMatch(/^ziee-memories-\d{4}-\d{2}-\d{2}\.csv$/)
    const lines = csv.body.split('\n')
    // Documented header row (MyMemoriesSection.tsx:341-351).
    expect(lines[0]).toBe(
      'id,content,kind,source,importance,confidence,recall_count,created_at,updated_at',
    )
    // The comma-bearing content must be RFC-4180 quoted, and present.
    expect(csv.body).toContain(`"${content}"`)
  })
})
