import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'

/**
 * E2E — deleting a citation from the library through the UI (audit gap
 * r2-510c99e164f2: thin citations E2E coverage).
 *
 * The existing 20-citations specs cover import, list, verification badges,
 * verify-all and export. None drives the per-card Delete flow:
 * `CitationCard` renders a `citations::manage`-gated, Popconfirm-wrapped
 * Delete button whose confirm calls `Stores.Citations.remove(id)` →
 * `DELETE /api/citations/{id}`. `library-real-backend.spec.ts` only calls
 * that endpoint over the API for *cleanup*, never through the rendered card.
 *
 * This spec uses ZERO page.route mocking of /api/citations* — it seeds via
 * the REAL `POST /api/citations/import` (a csl-only, identifier-less item
 * rests at `unverified` with no upstream), then drives the real card Delete
 * → Popconfirm → `DELETE /api/citations/{id}` and asserts the row leaves
 * the live list.
 */

test.describe('Citations library — delete via the card', () => {
  test.describe.configure({ retries: 1 })

  test('the card Delete button + Popconfirm fires the real DELETE and removes the row', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)

    const tag = Date.now().toString(36)
    const title = `Deletable Citation ${tag}`

    // --- Seed through the REAL import endpoint (no page.route) ---
    const importRes = await page.request.post(`${apiURL}/api/citations/import`, {
      headers: { Authorization: `Bearer ${adminToken}` },
      data: {
        items: [
          {
            csl: {
              type: 'article-journal',
              title,
              author: [{ family: 'Deletus', given: 'M.' }],
              issued: { 'date-parts': [[2022]] },
            },
          },
        ],
      },
    })
    expect(importRes.ok(), `import status ${importRes.status()}`).toBeTruthy()
    const report = (await importRes.json()) as {
      results: { entry_id: string | null; citation_key: string | null }[]
    }
    const result = report.results[0]
    expect(result?.entry_id, 'a real row was inserted').toBeTruthy()
    const entryId = result.entry_id!
    const citationKey = result.citation_key!
    expect(citationKey, 'the inserted entry has a citation key').toBeTruthy()

    let deleted = false
    try {
      // --- The UI lists it via the REAL GET /api/citations ---
      await page.goto(`${baseURL}/settings/citations`)
      await expect(
        page.getByRole('heading', { name: 'Citations' }),
      ).toBeVisible({ timeout: 10000 })
      await expect(page.getByText(title)).toBeVisible({ timeout: 10000 })

      // --- Drive the per-card Delete: button → Popconfirm → confirm ---
      // The card's Delete button is aria-labelled `Delete <citation_key>`.
      await page
        .getByRole('button', { name: `Delete ${citationKey}` })
        .click()

      // Confirm the Popconfirm and capture the REAL DELETE round-trip.
      const popconfirm = page.locator('.ant-popconfirm:visible').last()
      await expect(popconfirm).toBeVisible({ timeout: 5000 })

      const deleteResp = page.waitForResponse(
        r =>
          r.request().method() === 'DELETE' &&
          new RegExp(`/api/citations/${entryId}$`).test(r.url()),
        { timeout: 15000 },
      )
      // The Popconfirm's confirm button carries danger styling; "OK" is the
      // default confirm label.
      await popconfirm.getByRole('button', { name: 'OK' }).click()

      const resp = await deleteResp
      expect(resp.status(), 'DELETE /api/citations/{id} succeeded').toBeLessThan(300)
      deleted = true

      // The row leaves the live list (the store removed it + refetched).
      await expect(page.getByText(title)).toHaveCount(0, { timeout: 10000 })
    } finally {
      // Belt-and-suspenders: if the UI delete didn't land, clean up via API.
      if (!deleted) {
        await page.request
          .delete(`${apiURL}/api/citations/${entryId}`, {
            headers: { Authorization: `Bearer ${adminToken}` },
          })
          .catch(() => {})
      }
    }
  })
})
