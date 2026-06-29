import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'

/**
 * REAL-BACKEND citations coverage (the existing library.spec.ts is fully
 * route-mocked). This drives the production REST surface end-to-end via
 * `page.request` — NO route interception:
 *
 *   - import a CSL-only reference (no identifier → rests at `unverified`,
 *     deterministic, no resolver/network needed)
 *   - attach it into a project's reference list + scope-filter the library
 *     by `project_id` (the project-bibliography surface — previously 0 E2E)
 *   - detach + assert it leaves the project list but survives in the library
 *
 * Exercises the citations module ↔ project_bibliography M:N link for real.
 */

test.describe('Citations — project bibliography (real backend)', () => {
  test('import → attach to project → scope filter → detach', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    const auth = { Authorization: `Bearer ${token}` }

    // 1. Import a CSL-only book (no DOI/PMID) → unverified, no network.
    const importRes = await page.request.post(`${apiURL}/api/citations/import`, {
      headers: auth,
      data: {
        items: [
          {
            csl: {
              type: 'book',
              title: 'The Structure of Scientific Revolutions',
              author: [{ family: 'Kuhn', given: 'Thomas' }],
              issued: { 'date-parts': [[1962]] },
            },
          },
        ],
      },
    })
    expect(importRes.ok()).toBe(true)
    const importBody = await importRes.json()
    const result = importBody.results[0]
    expect(result.verification_status).toBe('unverified')
    const entryId: string = result.entry_id
    expect(entryId).toBeTruthy()

    // It is in the (unscoped) library.
    const lib = await page.request.get(`${apiURL}/api/citations`, { headers: auth })
    const libEntries = (await lib.json()).entries as { id: string }[]
    expect(libEntries.some(e => e.id === entryId)).toBe(true)

    // 2. Create a project and attach the entry into its reference list.
    const projRes = await page.request.post(`${apiURL}/api/projects`, {
      headers: auth,
      data: { name: `cite-proj-${Date.now()}` },
    })
    const projectId: string = (await projRes.json()).id

    const attach = await page.request.post(
      `${apiURL}/api/projects/${projectId}/citations`,
      { headers: auth, data: { entry_ids: [entryId] } },
    )
    expect(attach.ok()).toBe(true)

    // The project-scoped list contains the entry.
    const scoped = await page.request.get(
      `${apiURL}/api/citations?project_id=${projectId}`,
      { headers: auth },
    )
    const scopedEntries = (await scoped.json()).entries as { id: string }[]
    expect(scopedEntries.some(e => e.id === entryId)).toBe(true)

    // 3. Detach: it leaves the project list but stays in the library.
    const detach = await page.request.delete(
      `${apiURL}/api/projects/${projectId}/citations/${entryId}`,
      { headers: auth },
    )
    expect(detach.ok()).toBe(true)

    const scopedAfter = await page.request.get(
      `${apiURL}/api/citations?project_id=${projectId}`,
      { headers: auth },
    )
    const after = (await scopedAfter.json()).entries as { id: string }[]
    expect(after.some(e => e.id === entryId)).toBe(false)

    const libAfter = await page.request.get(`${apiURL}/api/citations`, {
      headers: auth,
    })
    const libAfterEntries = (await libAfter.json()).entries as { id: string }[]
    expect(libAfterEntries.some(e => e.id === entryId)).toBe(true)
  })
})
