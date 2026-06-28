import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'

// audit id all-7d848dd414aa — the citations library E2E uses page.route mocks
// for /api/citations*, so it never exercises the REAL backend pipeline. This
// spec hits the real backend: it imports a CSL-only citation (no DOI → no
// network resolution) via the real POST /api/citations/import, then loads
// /settings/citations with NO route mocks and asserts the entry renders from
// the real GET /api/citations.
test.describe('Citations library — real backend (no route mocks)', () => {
  test('an imported CSL citation persists and renders from the real backend', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)

    const uniqueTitle = `Real Backend Citation ${Date.now()}`
    const res = await fetch(`${apiURL}/api/citations/import`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${token}` },
      body: JSON.stringify({
        items: [
          {
            csl: {
              type: 'article-journal',
              title: uniqueTitle,
              author: [{ family: 'Doe', given: 'Jane' }],
              issued: { 'date-parts': [[2021]] },
            },
          },
        ],
      }),
    })
    expect(res.ok, `import failed: ${res.status} ${await res.text()}`).toBeTruthy()

    // Sanity: the real list endpoint already returns it (no UI, no mocks).
    const list = await (
      await fetch(`${apiURL}/api/citations`, {
        headers: { Authorization: `Bearer ${token}` },
      })
    ).json()
    const entries = Array.isArray(list) ? list : (list.entries ?? list.items ?? [])
    expect(
      entries.some((e: any) => e.title === uniqueTitle),
      'imported citation must persist in the real backend',
    ).toBeTruthy()

    // The settings page renders it from the real GET /api/citations.
    await page.goto(`${baseURL}/settings/citations`)
    await expect(page.getByText(uniqueTitle)).toBeVisible({ timeout: 30000 })
  })
})
