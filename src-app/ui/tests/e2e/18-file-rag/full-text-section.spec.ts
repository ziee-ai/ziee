import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'

// audit id all-e73fc0d29fc7 — FullTextSection (file-rag admin "Full-text search"
// card) had zero E2E coverage. It renders a form (enable toggle + RRF k +
// candidate multiplier + min rank) saved via the file-rag settings endpoint.
// This asserts the form renders and a changed value persists across reload
// (real backend, no mocks).
test.describe('Document RAG admin — Full-text search section', () => {
  test('renders the full-text form and persists an RRF k change', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await page.goto(`${baseURL}/settings/file-rag-admin`)

    // The card + its fields render.
    await expect(page.getByText('Full-text search').first()).toBeVisible({ timeout: 30000 })
    const ftsSwitch = page.getByLabel('Enable full-text search')
    await expect(ftsSwitch).toBeVisible()
    const rrfK = page.getByLabel('RRF k')
    await expect(rrfK).toBeVisible()

    // Change RRF k and save.
    await rrfK.fill('90')
    await page.getByRole('button', { name: 'Save' }).first().click()

    // Reload → the change persisted via the real backend.
    await page.goto(`${baseURL}/settings/file-rag-admin`)
    await expect(page.getByLabel('RRF k')).toHaveValue('90', { timeout: 15000 })
  })
})
