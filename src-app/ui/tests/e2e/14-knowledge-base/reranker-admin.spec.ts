import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { byTestId } from '../testid'

// TEST-42 (ITEM-38): the Reranker section on the Document-RAG admin page renders;
// with no rerank-capable model installed the Hub nudge shows; the controls
// (model picker / enable / candidate-k / save) are present. (rerank is an
// additive capability flag, not mutually exclusive with chat — see the drift.)
test.describe('Knowledge Base — reranker admin', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    await loginAsAdmin(page, testInfra.baseURL)
    await page.goto(`${testInfra.baseURL}/settings/file-rag-admin`)
  })

  test('reranker section + hub nudge render for an admin', async ({ page }) => {
    const card = byTestId(page, 'filerag-rerank-card')
    await expect(card).toBeVisible()

    // No rerank-capable model is installed in a fresh deployment → the Hub nudge.
    await expect(byTestId(page, 'filerag-rerank-hub-nudge')).toBeVisible()

    // The controls are present.
    await expect(byTestId(page, 'filerag-rerank-model-combobox')).toBeVisible()
    await expect(byTestId(page, 'filerag-rerank-enable-switch')).toBeVisible()
    await expect(byTestId(page, 'filerag-rerank-candidate-k-input')).toBeVisible()
  })

  test('candidate-k persists across reload', async ({ page }) => {
    await byTestId(page, 'filerag-rerank-candidate-k-input').fill('42')
    await byTestId(page, 'filerag-rerank-save').click()
    // reload and confirm the value round-tripped through the API.
    await page.reload()
    await expect(byTestId(page, 'filerag-rerank-candidate-k-input')).toHaveValue('42')
  })
})
