import type { Page } from '@playwright/test'
import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'

/**
 * E2E — the "Verify all" button on the citations library page.
 *
 * `handleVerifyAll` → `Citations.store.verifyAll()` → POST /api/citations/reverify,
 * then surfaces "Verified X; Y need attention." None of library.spec exercises
 * it. The list GET + the reverify POST (the external resolver boundary) are
 * mocked deterministically; the button wiring + result message are real.
 */

async function mockCitations(page: Page) {
  await page.route(/\/api\/citations(\?.*)?$/, async (route, req) => {
    if (req.method() === 'GET') {
      return route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({
          entries: [
            {
              id: crypto.randomUUID(),
              csl_json: { type: 'article-journal', title: 'A paper' },
              doi: '10.5555/known',
              pmid: null,
              pmcid: null,
              arxiv_id: null,
              title: 'A paper',
              year: 2021,
              citation_key: 'smith2021',
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
    return route.fallback()
  })

  await page.route(/\/api\/citations\/reverify$/, async (route, req) => {
    if (req.method() === 'POST') {
      return route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({
          results: [
            { verification_status: 'verified' },
            { verification_status: 'mismatch' },
          ],
        }),
      })
    }
    return route.fallback()
  })
}

test.describe('Citations — Verify all', () => {
  test('clicking "Verify all" runs reverify and reports the result', async ({
    page,
    testInfra,
  }) => {
    await mockCitations(page)
    await loginAsAdmin(page, testInfra.baseURL)

    await page.goto(`${testInfra.baseURL}/settings/citations`)
    await expect(page.getByText('smith2021')).toBeVisible({ timeout: 30000 })

    // "Verify all" is enabled (admin + ≥1 entry).
    const verifyAll = page.getByRole('button', { name: /Verify all/i })
    await expect(verifyAll).toBeEnabled()
    await verifyAll.click()

    // The reported result message (1 verified, 1 needs attention).
    await expect(
      page.getByText('Verified 1; 1 need attention.'),
    ).toBeVisible({ timeout: 10000 })
  })
})
