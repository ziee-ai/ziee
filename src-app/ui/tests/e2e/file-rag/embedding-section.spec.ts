import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { byTestId } from '../testid'

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
    const card = byTestId(page, 'filerag-embedding-card')
    await expect(card).toBeVisible({ timeout: 30000 })

    // No embedding-capable model on a fresh deployment → the empty-models
    // alert is shown and "Re-embed now" is disabled (handleReembed
    // early-returns when settings.embedding_model_id is null).
    await expect(byTestId(page, 'filerag-embedding-no-models-alert')).toBeVisible()
    await expect(byTestId(page, 'filerag-embedding-reembed-btn')).toBeDisabled()

    // Edit the cosine-distance threshold (InputNumber) to a new value.
    const cosine = byTestId(page, 'filerag-embedding-cosine')
    await cosine.click()
    await cosine.press('ControlOrMeta+a')
    await cosine.fill('0.35')

    // Toggle the semantic-search switch so the save carries a real change.
    await byTestId(page, 'filerag-embedding-switch').click()

    // Save and assert the real store→PUT round-trip succeeded.
    const saveResp = page.waitForResponse(
      r =>
        r.url().includes('/api/file-rag/admin-settings') &&
        r.request().method() === 'PUT' &&
        r.status() === 200,
    )
    await byTestId(page, 'filerag-embedding-save').click()
    await saveResp

    // The persisted cosine value survives a reload (real round-trip, not a
    // transient client edit).
    await page.reload()
    await expect(byTestId(page, 'filerag-embedding-cosine')).toHaveValue('0.35', {
      timeout: 30000,
    })
  })
})
