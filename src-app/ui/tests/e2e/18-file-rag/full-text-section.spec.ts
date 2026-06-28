import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'

// audit id all-e73fc0d29fc7 — FullTextSection (file-rag admin "Full-text search"
// card) had zero E2E coverage. It renders a form (enable toggle + RRF k +
// candidate multiplier + min rank) saved via the file-rag settings endpoint.
// This asserts the form renders and a changed value persists across reload
// (real backend, no mocks).
test.describe('Document RAG admin — Full-text search section', () => {
  test('renders the full-text form and persists an RRF k change', async ({ page, testInfra }) => {
/**
 * E2E — the Document-RAG "Full-text search" card (FullTextSection.tsx).
 *
 * Audit gap (0d3cc858122f): the full-text (lexical) tuning section — the
 * enable switch plus the RRF-k / candidate-multiplier / minimum-rank
 * InputNumbers and its Save button — had zero E2E coverage.
 *
 * This drives the card itself: it edits the RRF-k constant and toggles the
 * full-text enable switch, saves, asserts the real store→PUT
 * (PUT /api/file-rag/admin-settings) round-trip + success toast, and that the
 * edited value survives a reload (a real persisted change, not a transient
 * client edit). Only the UI is exercised — no embedder needed (full-text is
 * the no-embedding-model day-one search arm).
 */

test.describe('Document RAG — full-text search section', () => {
  test('editing RRF k + full-text toggle saves and persists across reload', async ({
    page,
    testInfra,
  }) => {
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
    // Scope every interaction to the Full-text card so its Save button isn't
    // confused with the other section cards' Save buttons.
    const card = page
      .locator('.ant-card')
      .filter({ hasText: 'Full-text search' })
    await expect(card).toBeVisible({ timeout: 30000 })

    // Read the current RRF-k value and pick a deliberately-different one so the
    // save carries a real change (default is 60 per the RRF paper).
    const rrfK = card.getByRole('spinbutton', { name: /RRF k/i })
    await expect(rrfK).toBeVisible()
    const newRrfK = (await rrfK.inputValue()) === '45' ? '50' : '45'

    await rrfK.click()
    await rrfK.press('ControlOrMeta+a')
    await rrfK.fill(newRrfK)

    // Toggle the full-text enable switch so the save carries a second real
    // change (the lexical arm on/off).
    await card.getByRole('switch', { name: 'Enable full-text search' }).click()

    // Save and assert the real store→PUT round-trip succeeded.
    const saveResp = page.waitForResponse(
      r =>
        r.url().includes('/api/file-rag/admin-settings') &&
        r.request().method() === 'PUT' &&
        r.status() === 200,
    )
    await card.getByRole('button', { name: 'Save' }).click()
    await saveResp

    await expect(page.getByText('Full-text settings saved.')).toBeVisible({
      timeout: 30000,
    })

    // The persisted RRF-k value survives a reload (real round-trip).
    await page.reload()
    await expect(
      page
        .locator('.ant-card')
        .filter({ hasText: 'Full-text search' })
        .getByRole('spinbutton', { name: /RRF k/i }),
    ).toHaveValue(newRrfK, { timeout: 30000 })
  })
})
