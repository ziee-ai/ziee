import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'

/**
 * E2E — the Document-RAG master "Document search" card (EnableSection.tsx).
 *
 * Audit gap: admin-surface.spec round-trips the Chunking card's Save but
 * never the master EnableSection (deployment-wide enable Switch +
 * `default_top_k` → save). This edits top-K and saves, asserting the
 * "Document search settings saved." toast (real store→PUT round-trip).
 */

test.describe('Document RAG — master enable section', () => {
  test('editing default top-K and saving shows the success toast', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await page.goto(`${baseURL}/settings/file-rag-admin`)

    // The master "Document search" card.
    const card = page.locator(
      '.ant-card:has([aria-label="Enable Document RAG deployment-wide"])',
    )
    await expect(card).toBeVisible({ timeout: 30000 })

    const topK = card.getByLabel('Default top-K')
    await topK.click()
    await topK.press('ControlOrMeta+a')
    await topK.fill('8')

    await card.getByRole('button', { name: 'Save' }).click()
    await expect(
      page.getByText('Document search settings saved.'),
    ).toBeVisible({ timeout: 30000 })
  })
})
