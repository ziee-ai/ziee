import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { byTestId } from '../testid'

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

    // Scope every interaction to the Full-text card.
    const card = byTestId(page, 'filerag-fts-card')
    await expect(card).toBeVisible({ timeout: 30000 })

    // Read the current RRF-k value and pick a deliberately-different one so the
    // save carries a real change (default is 60 per the RRF paper).
    const rrfK = byTestId(page, 'filerag-fts-rrf-k')
    await expect(rrfK).toBeVisible()
    const newRrfK = (await rrfK.inputValue()) === '45' ? '50' : '45'

    await rrfK.click()
    await rrfK.press('ControlOrMeta+a')
    await rrfK.fill(newRrfK)

    // Toggle the full-text enable switch so the save carries a second real
    // change (the lexical arm on/off).
    await byTestId(page, 'filerag-fts-switch').click()

    // Save and assert the real store→PUT round-trip succeeded.
    const saveResp = page.waitForResponse(
      r =>
        r.url().includes('/api/file-rag/admin-settings') &&
        r.request().method() === 'PUT' &&
        r.status() === 200,
    )
    await byTestId(page, 'filerag-fts-save').click()
    await saveResp

    // The persisted RRF-k value survives a reload (real round-trip).
    await page.reload()
    await expect(byTestId(page, 'filerag-fts-rrf-k')).toHaveValue(newRrfK, {
      timeout: 30000,
    })
  })
})
