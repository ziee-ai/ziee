import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'

/**
 * ProjectsNavWidget — the sidebar widget listing the user's projects with a
 * row→detail navigation and an "All projects" footer button. No prior E2E
 * exercised the widget's navigation.
 */
test.describe('Projects - sidebar nav widget', () => {
  test('a project row navigates to its detail page; "All projects" opens the list', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)

    const created = await page.request.post(`${apiURL}/api/projects`, {
      headers: { Authorization: `Bearer ${token}` },
      data: { name: 'Sidebar Project' },
    })
    const projectId: string = (await created.json()).id

    // Land on the app shell — the sidebar widget mounts + self-fetches.
    await page.goto(`${baseURL}/`)

    // The widget renders the project as a role="button" row.
    const row = page.getByRole('button', { name: 'Open project Sidebar Project' })
    await expect(row).toBeVisible({ timeout: 30000 })
    await row.click()
    await expect(page).toHaveURL(new RegExp(`/projects/${projectId}$`))

    // Back to the shell, the "All projects" footer button opens the list page.
    await page.goto(`${baseURL}/`)
    await page.getByRole('button', { name: 'All projects' }).click()
    await expect(page).toHaveURL(/\/projects$/)
    await expect(
      page.getByRole('heading', { name: /projects/i }).first(),
    ).toBeVisible()
  })
})
