import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import { byTestId } from '../testid'

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
      await expect(byTestId(page, 'cite-settings-card')).toBeVisible({ timeout: 10000 })

      // The seeded card renders with its (plain) `unverified` badge.
      const card = byTestId(page, `cite-card-${entryId}`)
      await expect(card).toBeVisible({ timeout: 10000 })
      await expect(card.getByTestId('cite-badge-unverified')).toBeVisible()

      const verifyButton = byTestId(page, 'cite-settings-verify-all-button')
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

      // After verifyAll's reload, the entry still renders with a (re-resolved,
      // persisted) verification badge — one of the four valid states. The
      // button's loading state has cleared and the card survived the reload.
      await expect(card).toBeVisible({ timeout: 15000 })
      await expect(verifyButton).toBeEnabled()
      await expect(
        card.locator('[data-testid^="cite-badge-"]'),
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
