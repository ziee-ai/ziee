import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'

/**
 * E2E — the Document-RAG "Embedding (semantic search)" card (EmbeddingSection.tsx).
 *
 * Audit gap (all-fbd4a07ed3c5): the embedding section — semantic-search
 * switch, the embedding-model picker, the cosine-distance threshold, and
 * the "Re-embed now" affordance — had zero E2E coverage. admin-surface.spec
 * only asserts the card + picker *render*; enable-section-save covers a
 * DIFFERENT card (the master EnableSection).
 *
 * This drives the Embedding card itself: it edits the cosine threshold and
 * toggles semantic search, saves, and asserts the real store→PUT
 * (PUT /api/file-rag/admin-settings) success toast. It also asserts the
 * model-picker gating: on a fresh deployment with no embedding-capable
 * model registered, the picker shows its empty placeholder and the
 * "Re-embed now" button is disabled (handleReembed early-returns without a
 * model). Only the UI is exercised — no real embedder is needed.
 */

test.describe('Document RAG — embedding (semantic search) section', () => {
  test('editing cosine threshold + semantic toggle saves; re-embed gated on model', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await page.goto(`${baseURL}/settings/file-rag-admin`)

    // Scope every interaction to the Embedding card so its Save button isn't
    // confused with the other section cards' Save buttons.
    const card = page
      .locator('.ant-card')
      .filter({ hasText: 'Embedding (semantic search)' })
    await expect(card).toBeVisible({ timeout: 30000 })

    // No embedding-capable model on a fresh deployment → the picker carries
    // its empty placeholder and "Re-embed now" is disabled (handleReembed
    // early-returns when settings.embedding_model_id is null).
    await expect(
      card.getByText('No embedding-capable models found.'),
    ).toBeVisible()
    await expect(
      card.getByRole('button', { name: 'Re-embed now' }),
    ).toBeDisabled()

    // Edit the cosine-distance threshold (InputNumber) to a new value.
    const cosine = card.getByRole('spinbutton', {
      name: /Cosine distance threshold/i,
    })
    await cosine.click()
    await cosine.press('ControlOrMeta+a')
    await cosine.fill('0.35')

    // Toggle the semantic-search switch so the save carries a real change.
    await card.getByRole('switch', { name: 'Enable semantic search' }).click()

    // Save and assert the real store→PUT round-trip succeeded. Because no
    // model changed, the unchanged-model success copy is shown.
    const saveResp = page.waitForResponse(
      r =>
        r.url().includes('/api/file-rag/admin-settings') &&
        r.request().method() === 'PUT' &&
        r.status() === 200,
    )
    await card.getByRole('button', { name: 'Save' }).click()
    await saveResp

    await expect(page.getByText('Embedding settings saved.')).toBeVisible({
      timeout: 30000,
    })

    // The persisted cosine value survives a reload (real round-trip, not a
    // transient client edit).
    await page.reload()
    await expect(
      page
        .locator('.ant-card')
        .filter({ hasText: 'Embedding (semantic search)' })
        .getByRole('spinbutton', { name: /Cosine distance threshold/i }),
    ).toHaveValue('0.35', { timeout: 30000 })
  })
})
