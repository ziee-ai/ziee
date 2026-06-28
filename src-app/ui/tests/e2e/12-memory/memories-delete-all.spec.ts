import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'

// audit id all-482e3d3c2626 — MyMemoriesSection bulk "Delete all" (Popconfirm →
// Stores.Memories.removeAll → DELETE) had no UI test. Seed memories via the real
// API, then delete them all through the UI and assert the success toast + empty
// state.
test.describe('My memories — delete all', () => {
  test('Delete all removes every memory', async ({ page, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)

    // Seed two memories through the real API.
    for (const content of ['Memory one about Rust', 'Memory two about Tokio']) {
      const r = await fetch(`${apiURL}/api/memories`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${token}` },
        body: JSON.stringify({ content, kind: 'fact', importance: 50 }),
      })
      if (!r.ok) throw new Error(`seed memory: ${r.status} ${await r.text()}`)
    }

    await page.goto(`${baseURL}/settings/memory`)
    await expect(page.getByText('Memory one about Rust')).toBeVisible({ timeout: 30000 })

    // Bulk delete via the "Delete all" button + Popconfirm.
    await page.getByRole('button', { name: 'Delete all' }).click()
    await page.getByRole('button', { name: 'Delete', exact: true }).click()

    // Success toast + the memories are gone.
    await expect(page.getByText(/Deleted \d+ memories/)).toBeVisible({ timeout: 10000 })
    await expect(page.getByText('Memory one about Rust')).toHaveCount(0, { timeout: 10000 })
  })
})
