import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'

// audit id all-482e3d3c2626 — MyMemoriesSection bulk "Delete all" (Confirm →
// Stores.Memories.removeAll → DELETE) had no UI test. Seed memories via the real
// API, then delete them all through the UI and assert the seeded rows are gone.
test.describe('My memories — delete all', () => {
  test('Delete all removes every memory', async ({ page, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)

    // Seed two memories through the real API, capturing their ids so we can
    // target the rendered rows by their stable `data-memory-id`.
    const ids: string[] = []
    for (const content of ['Memory one about Rust', 'Memory two about Tokio']) {
      const r = await fetch(`${apiURL}/api/memories`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${token}` },
        body: JSON.stringify({ content, kind: 'fact', importance: 50 }),
      })
      if (!r.ok) throw new Error(`seed memory: ${r.status} ${await r.text()}`)
      ids.push((await r.json()).id as string)
    }

    await page.goto(`${baseURL}/settings/memory`)
    await expect(page.locator(`[data-memory-id="${ids[0]}"]`)).toBeVisible({ timeout: 30000 })

    // Bulk delete via the "Delete all" button + Confirm dialog.
    await byTestId(page, 'memory-delete-all-btn').click()
    await byTestId(page, 'memory-delete-all-confirm-confirm').click()

    // The seeded memories are gone.
    await expect(page.locator(`[data-memory-id="${ids[0]}"]`)).toHaveCount(0, { timeout: 10000 })
    await expect(page.locator(`[data-memory-id="${ids[1]}"]`)).toHaveCount(0)
  })
})
