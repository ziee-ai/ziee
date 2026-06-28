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
  })
})
