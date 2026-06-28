import type { Page } from '@playwright/test'
import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'

/**
 * The "Verify all" action on Settings → Citations re-verifies the whole library
 * (POST /api/citations/reverify) and reports the outcome ("Verified N; M need
 * attention."). The existing library spec covers import/badges/export/delete but
 * never the bulk re-verify control. Only the reverify HTTP boundary is mocked;
 * the page→store→summary-toast wiring is the behavior under test.
 */

async function mockApi(page: Page) {
  // The library list (one already-verified, one unverified entry).
  await page.route(/\/api\/citations(\?.*)?$/, async (route, req) => {
    if (req.method() === 'GET') {
      return route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({
          entries: [
            {
              id: crypto.randomUUID(),
              csl_json: { type: 'article-journal', title: 'Already verified' },
              doi: '10.5555/ok',
              pmid: null,
              pmcid: null,
              arxiv_id: null,
              title: 'Already verified',
              year: 2020,
              citation_key: 'ok2020',
              verification_status: 'verified',
              verified_at: new Date().toISOString(),
              source: 'doi',
              created_at: new Date().toISOString(),
              updated_at: new Date().toISOString(),
            },
            {
              id: crypto.randomUUID(),
              csl_json: { type: 'article-journal', title: 'Needs a recheck' },
              doi: '10.5555/recheck',
              pmid: null,
              pmcid: null,
              arxiv_id: null,
              title: 'Needs a recheck',
              year: 2019,
              citation_key: 'recheck2019',
              verification_status: 'unverified',
              verified_at: null,
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

  // The bulk re-verify result: 1 verified, 1 not_found.
  await page.route(/\/api\/citations\/reverify$/, async (route, req) => {
    if (req.method() === 'POST') {
      return route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({
          results: [
            { input: '10.5555/ok', verification_status: 'verified' },
            { input: '10.5555/recheck', verification_status: 'not_found', reason: 'unresolved' },
          ],
        }),
      })
    }
    return route.continue()
  })
}

test.describe('Citations — Verify all', () => {
  test.describe.configure({ retries: 2 })

  test('re-verifies the library and reports the outcome', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await mockApi(page)

    await page.goto(`${baseURL}/settings/citations`)
    await expect(page.getByRole('heading', { name: 'Citations' })).toBeVisible({
      timeout: 10000,
    })

    // The reverify call must fire when "Verify all" is clicked.
    const reverify = page.waitForRequest(
      (req) => /\/api\/citations\/reverify$/.test(req.url()) && req.method() === 'POST',
    )
    await page.getByRole('button', { name: 'Verify all' }).click()
    await reverify

    // The summary toast reflects the batch report (1 verified, 1 needs attention).
    await expect(page.getByText('Verified 1; 1 need attention.')).toBeVisible({
      timeout: 10000,
    })
  })
})
