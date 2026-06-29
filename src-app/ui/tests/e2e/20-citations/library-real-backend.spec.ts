import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'

/**
 * E2E — the citations library against the REAL backend pipeline
 * (audit gap all-fffc354d355c).
 *
 * The sibling `library.spec.ts` route-mocks GET/POST /api/citations,
 * /import and /export with `page.route`, so it asserts the UI contract
 * but never exercises the real add → list → export path through the
 * server + database. This spec uses ZERO page.route mocking of any
 * /api/citations* endpoint — every byte is served by the live backend.
 *
 * Seeding goes through the REAL `POST /api/citations/import` endpoint
 * with a CSL-JSON item that carries NO identifier (no DOI/PMID/arXiv).
 * Per `citations/resolve.rs::resolve_input`, a csl-only item with no DOI
 * resolves to `unverified` WITHOUT any upstream network call — so the
 * real add/list/export pipeline runs deterministically with no resolver
 * dependency (the resolve-against-doi.org path is covered by the backend
 * mock-resolve tier).
 */

test.describe('Citations library — real backend pipeline', () => {
  test.describe.configure({ retries: 1 })

  test('import (csl, no DOI) → real list renders it → real export contains it', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)

    // A unique, identifier-less title so this run never collides with
    // another and the assertions can't accidentally match seed data.
    const tag = Date.now().toString(36)
    const title = `Real Backend Citation ${tag}`

    // --- Seed through the REAL import endpoint (no page.route) ---
    // csl-only, no DOI → backend stores it as `unverified` with no upstream.
    const importRes = await page.request.post(`${apiURL}/api/citations/import`, {
      headers: { Authorization: `Bearer ${adminToken}` },
      data: {
        items: [
          {
            csl: {
              type: 'article-journal',
              title,
              author: [{ family: 'Tester', given: 'E.' }],
              issued: { 'date-parts': [[2023]] },
            },
          },
        ],
      },
    })
    expect(importRes.ok(), `import status ${importRes.status()}`).toBeTruthy()
    const report = (await importRes.json()) as {
      results: { entry_id: string | null; verification_status: string }[]
    }
    const result = report.results[0]
    expect(result, 'import returned a result row').toBeTruthy()
    // The anti-fabrication contract: an identifier-less item rests at
    // `unverified` (a real, legitimate state — not `not_found`), and IS stored.
    expect(result.verification_status).toBe('unverified')
    expect(result.entry_id, 'a real row was inserted').toBeTruthy()
    const entryId = result.entry_id!

    try {
      // --- The UI lists it via the REAL GET /api/citations ---
      await page.goto(`${baseURL}/settings/citations`)
      await expect(
        page.getByRole('heading', { name: 'Citations' }),
      ).toBeVisible({ timeout: 10000 })

      // The card text + its plain (uncolored) `unverified` tag come straight
      // from the database via the real list endpoint.
      await expect(page.getByText(title)).toBeVisible({ timeout: 10000 })
      await expect(
        page.getByText('unverified', { exact: true }).first(),
      ).toBeVisible()

      // --- Export via the REAL GET /api/citations/export (RIS = pure-Rust
      // writer, no pandoc) and assert the downloaded body carries the title ---
      const downloadPromise = page.waitForEvent('download')
      await page.getByRole('button', { name: 'Export' }).click()
      await page.getByText('RIS (.ris)').click()
      const download = await downloadPromise
      expect(download.suggestedFilename()).toContain('citations')

      const stream = await download.createReadStream()
      const chunks: Buffer[] = []
      for await (const c of stream) chunks.push(c as Buffer)
      const body = Buffer.concat(chunks).toString('utf8')
      // RIS encodes the title in a `TI  - <title>` line; at minimum the
      // unique title must be present in the real exported output.
      expect(body).toContain(title)
    } finally {
      // Clean up the seeded row via the real DELETE endpoint.
      await page.request
        .delete(`${apiURL}/api/citations/${entryId}`, {
          headers: { Authorization: `Bearer ${adminToken}` },
        })
        .catch(() => {})
    }
  })
})
