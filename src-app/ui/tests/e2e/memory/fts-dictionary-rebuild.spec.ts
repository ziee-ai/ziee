import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { byTestId } from '../testid'

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
    const ftsCard = byTestId(page, 'memory-fts-card')
    await expect(ftsCard).toBeVisible({ timeout: 20000 })

    // Change the Dictionary Select from the default ('simple') to 'english'.
    // Kit Select emits options as `${selectTestid}-opt-${value}`.
    await byTestId(page, 'memory-fts-dictionary-select').click()
    await byTestId(page, 'memory-fts-dictionary-select-opt-english').click()

    // Save → the dictionary changed, so a confirm Modal is shown (NOT an
    // in-place save).
    await byTestId(page, 'memory-fts-save-btn').click()
    const modal = byTestId(page, 'memory-fts-rebuild-dialog')
    await expect(modal).toBeVisible()

    // Confirm → the rebuild POST fires with the new dictionary; the dialog
    // closes once the rebuild request resolves.
    await byTestId(page, 'memory-fts-rebuild-confirm-btn').click()
    await expect(modal).toBeHidden({ timeout: 10000 })
    expect(rebuildBody?.['dictionary']).toBe('english')
  })
})
