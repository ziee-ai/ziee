import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import { byTestId } from '../testid'

/**
 * ProjectBibliographyManagePanel (citations project-extension, registered as
 * the "References" knowledge kind). The project detail page's "Manage" drawer
 * stacks every knowledge kind's managePanel, so the References panel renders
 * there. Previously zero E2E coverage.
 */
test.describe('Projects - bibliography manage panel', () => {
  test('References panel shows empty state then a seeded reference', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    const auth = { Authorization: `Bearer ${token}` }

    const proj = await page.request.post(`${apiURL}/api/projects`, {
      headers: auth,
      data: { name: 'Bibliography Project' },
    })
    const projectId: string = (await proj.json()).id

    // EMPTY: open the project, open the knowledge Manage drawer → the
    // References panel shows its empty state + Import button.
    await page.goto(`${baseURL}/projects/${projectId}`)
    await byTestId(page, 'project-knowledge-manage-button').click()
    const drawer = page.getByRole('dialog')
    await expect(drawer).toBeVisible()
    await expect(byTestId(drawer, 'cite-bib-panel-empty')).toBeVisible()
    await expect(
      byTestId(drawer, 'cite-bib-panel-import-button'),
    ).toBeVisible()

    // SEED a reference directly into the project (CSL-only book → unverified,
    // no network) and confirm the panel reflects it after a reload.
    const imp = await page.request.post(`${apiURL}/api/citations/import`, {
      headers: auth,
      data: {
        project_id: projectId,
        items: [
          {
            csl: {
              type: 'book',
              title: 'A Project-Scoped Reference',
              author: [{ family: 'Doe', given: 'Jane' }],
              issued: { 'date-parts': [[2019]] },
            },
          },
        ],
      },
    })
    expect(imp.ok()).toBe(true)

    await page.goto(`${baseURL}/projects/${projectId}`)
    await byTestId(page, 'project-knowledge-manage-button').click()
    const drawer2 = page.getByRole('dialog')
    await expect(drawer2).toBeVisible()
    // The seeded reference now renders as a CitationCard (keyed by entry
    // id) and the empty state is gone — exactly one card for the one ref.
    await expect(byTestId(drawer2, 'cite-bib-panel-empty')).toHaveCount(0)
    // Exactly one CitationCard rendered — count the per-entry delete
    // button (uniquely one per reference) to avoid matching the card's
    // other `cite-card-*` sub-element testids.
    await expect(
      drawer2.locator('[data-testid^="cite-card-delete-button-"]'),
    ).toHaveCount(1, { timeout: 10000 })
  })
})
