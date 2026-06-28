import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'

/**
 * E2E — the "Verify all" button on the Citations settings page
 * (audit gap all-108b1e8893ca).
 *
 * `library.spec.ts` route-mocks the citations API and never clicks
 * "Verify all"; nothing exercised `CitationsSettingsPage.handleVerifyAll`
 * → `Stores.Citations.verifyAll()` → the real `POST /api/citations/reverify`
 * → reload → re-rendered verification badges.
 *
 * This drives the REAL flow with ZERO mocking of any /api/citations*
 * endpoint. Seeding goes through the real `POST /api/citations/import`
 * with a CSL-only, identifier-less item: per `citations/resolve.rs`, an
 * item with no DOI/PMID rests at `unverified` with NO upstream network
 * call, so the seed is deterministic. Clicking "Verify all" then fires
 * the real reverify endpoint (the behaviour under test); we assert the
 * real round-trip completes, the UI surfaces the completion report, and
 * the entry's verification badge re-renders — none of it mocked.
 */

test.describe('Citations — Verify all', () => {
  test.describe.configure({ retries: 1 })

  test('clicking "Verify all" fires the real reverify endpoint and re-renders the badge', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)

    const tag = Date.now().toString(36)
    const title = `Verify All Citation ${tag}`

    // --- Seed one identifier-less citation through the REAL import endpoint
    // (no page.route). csl-only, no DOI → stored `unverified`, no upstream. ---
    const importRes = await page.request.post(`${apiURL}/api/citations/import`, {
      headers: { Authorization: `Bearer ${adminToken}` },
      data: {
        items: [
          {
            csl: {
              type: 'article-journal',
              title,
              author: [{ family: 'Verifier', given: 'A.' }],
              issued: { 'date-parts': [[2022]] },
            },
          },
        ],
      },
    })
    expect(importRes.ok(), `import status ${importRes.status()}`).toBeTruthy()
    const report = (await importRes.json()) as {
      results: { entry_id: string | null; verification_status: string }[]
    }
    const entryId = report.results[0]?.entry_id
    expect(entryId, 'a real row was inserted').toBeTruthy()

    try {
      await page.goto(`${baseURL}/settings/citations`)
      await expect(
        page.getByRole('heading', { name: 'Citations' }),
      ).toBeVisible({ timeout: 10000 })

      // The seeded card renders with its (plain) `unverified` badge.
      await expect(page.getByText(title)).toBeVisible({ timeout: 10000 })
      await expect(
        page.getByText('unverified', { exact: true }).first(),
      ).toBeVisible()

      const verifyButton = page.getByRole('button', { name: 'Verify all' })
      await expect(verifyButton).toBeEnabled()

      // --- Click "Verify all" and assert the REAL reverify round-trip ---
      const reverifyResponse = page.waitForResponse(
        r =>
          r.url().includes('/api/citations/reverify') &&
          r.request().method() === 'POST',
        { timeout: 30000 },
      )
      await verifyButton.click()
      const resp = await reverifyResponse
      expect(resp.ok(), `reverify status ${resp.status()}`).toBeTruthy()

      // The store surfaces the completion report via an antd message toast
      // ("Verified N; M need attention.") — proves handleVerifyAll ran to
      // completion (not an error toast) off the real endpoint's BatchReport.
      await expect(
        page.getByText(/Verified \d+; \d+ need attention\./),
      ).toBeVisible({ timeout: 15000 })

      // After verifyAll's reload, the entry still renders with a (re-resolved,
      // persisted) verification badge — one of the four valid states. The
      // button's loading state has cleared and the card survived the reload.
      await expect(page.getByText(title)).toBeVisible()
      await expect(verifyButton).toBeEnabled()
      await expect(
        page
          .getByText(/^(verified|unverified|not found|mismatch)$/)
          .first(),
      ).toBeVisible()
    } finally {
      await page.request
        .delete(`${apiURL}/api/citations/${entryId}`, {
          headers: { Authorization: `Bearer ${adminToken}` },
        })
        .catch(() => {})
    }
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
              csl_json: { type: 'article-journal', title: 'A paper' },
              doi: '10.5555/known',
              pmid: null,
              pmcid: null,
              arxiv_id: null,
              title: 'A paper',
              year: 2021,
              citation_key: 'smith2021',
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
    return route.fallback()
  })

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
            { verification_status: 'verified' },
            { verification_status: 'mismatch' },
            { input: '10.5555/ok', verification_status: 'verified' },
            { input: '10.5555/recheck', verification_status: 'not_found', reason: 'unresolved' },
          ],
        }),
      })
    }
    return route.fallback()
    return route.continue()
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
