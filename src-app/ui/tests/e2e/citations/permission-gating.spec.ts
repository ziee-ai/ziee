import type { Page } from '@playwright/test'
import { test, expect } from '../../fixtures/test-context'
import { loginWithPerms } from '../permissions/fixtures'
import { Permissions } from '../../../src/api-client/permissions'
import { byTestId } from '../testid'

/**
 * E2E — citations `use` vs `manage` permission gating of UI actions.
 *
 * `CitationsSettingsPage` hides "Import" and disables "Verify all" without
 * `citations::manage`; `CitationCard` hides the per-entry delete button. The
 * existing library.spec always logs in as admin (who holds `*`), so the
 * use-only path was never exercised. This logs in with `citations::use` only.
 *
 * The citations list GET is route-mocked (external boundary) so a card renders
 * deterministically; the gating itself is real (driven by usePermission).
 */

const ENTRY_ID = '00000000-0000-4000-8000-0000000c1701'

async function mockOneEntry(page: Page) {
  await page.route(/\/api\/citations(\?.*)?$/, async (route, req) => {
    if (req.method() === 'GET') {
      return route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({
          entries: [
            {
              id: ENTRY_ID,
              csl_json: { type: 'article-journal', title: 'A paper' },
              doi: '10.5555/known',
              pmid: null,
              pmcid: null,
              arxiv_id: null,
              title: 'A paper',
              year: 2021,
              citation_key: 'smith2021',
              verification_status: 'verified',
              verified_at: new Date().toISOString(),
              source: 'doi',
              created_at: new Date().toISOString(),
              updated_at: new Date().toISOString(),
            },
          ],
        }),
      })
    }
    return route.continue()
  })
}

test.describe('Citations — use vs manage gating', () => {
  test('a use-only user cannot Import, Verify-all, or delete entries', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await mockOneEntry(page)

    // citations::use but NOT citations::manage.
    await loginWithPerms(page, baseURL, apiURL, [Permissions.CitationsUse])

    await page.goto(`${baseURL}/settings/citations`)
    // The card list renders (the seeded entry's card is shown).
    await expect(byTestId(page, `cite-card-${ENTRY_ID}`)).toBeVisible({ timeout: 30000 })

    // Import is hidden for a non-manage user.
    await expect(byTestId(page, 'cite-settings-import-button')).toHaveCount(0)

    // "Verify all" is present but disabled.
    await expect(byTestId(page, 'cite-settings-verify-all-button')).toBeDisabled()

    // The per-entry delete affordance is hidden.
    await expect(byTestId(page, `cite-card-delete-button-${ENTRY_ID}`)).toHaveCount(0)
  })
})
