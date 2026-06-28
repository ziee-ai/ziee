import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'

/**
 * E2E — Project Bibliography Manage Panel "Import into project" button opens the
 * ImportCitationsModal (ProjectBibliographyManagePanel.tsx:57-63). Reached via a
 * project's knowledge "Manage" drawer (the References knowledge-kind panel).
 */

test.describe('Citations — project bibliography import', () => {
  test('"Import into project" opens the import-citations modal', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)

    const proj = await page.request.post(`${apiURL}/api/projects`, {
      headers: { Authorization: `Bearer ${token}` },
      data: { name: `Bib Import ${Date.now()}` },
    })
    expect(proj.ok()).toBeTruthy()
    const projectId = (await proj.json()).id as string

    await page.goto(`${baseURL}/projects/${projectId}`)

    // Open the knowledge "Manage" drawer (hosts the References manage panel).
    await page.getByRole('button', { name: /manage knowledge files/i }).click()
    await page
      .locator('.ant-drawer.ant-drawer-open')
      .waitFor({ state: 'visible' })

    // The References panel's "Import into project" button opens the modal.
    await page.getByRole('button', { name: 'Import into project' }).click()
    const modal = page.getByRole('dialog', { name: 'Import citations' })
    await expect(modal).toBeVisible({ timeout: 10000 })
    await expect(modal.getByRole('button', { name: 'Import + verify' })).toBeVisible()
  })
})
