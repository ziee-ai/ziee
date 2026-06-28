import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'

/**
 * Admin FullTextSearchSection — the dictionary-swap rebuild flow
 * (FullTextSearchSection.tsx). Changing the FTS `dictionary` Select can't be
 * an in-place PUT (the GENERATED `content_tsv` column bakes the dictionary in),
 * so saving a changed dictionary opens a confirm Modal and, on confirm, POSTs
 * /api/memory/admin/fts/rebuild. The real rebuild rewrites every row, so we
 * mock ONLY that POST and assert the UI contract: confirm Modal → rebuild
 * request carries the new dictionary → "rebuild started" info toast.
 */
test.describe('Memory admin — FTS dictionary rebuild', () => {
  test('changing the dictionary confirms + triggers a rebuild', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)

    let rebuildBody: Record<string, unknown> | null = null
    await page.route(
      /\/api\/memory\/admin\/fts\/rebuild$/,
      async (route, req) => {
        if (req.method() === 'POST') {
          rebuildBody = JSON.parse(req.postData() ?? '{}')
          return route.fulfill({
            status: 200,
            contentType: 'application/json',
            body: JSON.stringify({ in_progress: true, dictionary: 'english' }),
          })
        }
        return route.continue()
      },
    )

    await page.goto(`${baseURL}/settings/memory-admin`)
    const ftsCard = page.locator('.ant-card', { hasText: 'Full-text search' })
    await expect(ftsCard).toBeVisible({ timeout: 20000 })

    // Change the Dictionary Select from the default ('simple') to 'english'.
    await ftsCard.locator('.ant-select').first().click()
    await page.getByRole('option', { name: 'english', exact: true }).click()

    // Save → the dictionary changed, so a confirm Modal is shown (NOT an
    // in-place save).
    await ftsCard.getByRole('button', { name: 'Save' }).click()
    const modal = page.getByRole('dialog')
    await expect(
      modal.getByText('Rebuild the full-text search index?'),
    ).toBeVisible()

    // Confirm → the rebuild POST fires with the new dictionary + an info toast.
    await modal.getByRole('button', { name: 'Rebuild' }).click()
    await expect(
      page.getByText(/Full-text search rebuild started/),
    ).toBeVisible({ timeout: 10000 })
    expect(rebuildBody?.['dictionary']).toBe('english')
  })
})
