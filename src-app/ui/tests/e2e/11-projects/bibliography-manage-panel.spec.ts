import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'

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
    await page
      .getByRole('button', { name: /manage knowledge files/i })
      .click()
    const drawer = page.locator('.ant-drawer.ant-drawer-open')
    await expect(drawer).toBeVisible()
    await expect(
      drawer.getByText('No references in this project yet.'),
    ).toBeVisible()
    await expect(
      drawer.getByRole('button', { name: 'Import into project' }),
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
    await page
      .getByRole('button', { name: /manage knowledge files/i })
      .click()
    const drawer2 = page.locator('.ant-drawer.ant-drawer-open')
    await expect(drawer2).toBeVisible()
    await expect(
      drawer2.getByText('A Project-Scoped Reference'),
    ).toBeVisible({ timeout: 10000 })
  })
})
