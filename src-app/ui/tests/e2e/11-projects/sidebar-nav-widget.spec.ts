import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import { byTestId } from '../testid'

/**
 * E2E — ProjectsNavWidget (the LeftSidebar "Projects" widget).
 * audit id 070db752 — the widget's navigation (per-project "Open project X"
 * rows + the "All projects" link, ProjectsNavWidget.tsx) had no E2E coverage;
 * the existing 11-projects/sidebar-menu.spec.ts covers a DIFFERENT surface (the
 * RecentConversationsWidget 3-dot menu).
 */

async function seedProject(apiURL: string, token: string, name: string): Promise<string> {
  const res = await fetch(`${apiURL}/api/projects`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${token}` },
    body: JSON.stringify({ name }),
  })
  if (!res.ok) throw new Error(`seed project failed: ${res.status} ${await res.text()}`)
  return (await res.json()).id
}

test.describe('Projects — sidebar nav widget', () => {
  test.describe.configure({ retries: 2 })

  test('clicking a project row navigates to its detail page', async ({ page, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    const name = `e2e-navw-${Date.now()}`
    const projectId = await seedProject(apiURL, token, name)

    // Land on the app shell so the LeftSidebar (and its Projects widget) renders.
    await page.goto(`${baseURL}/`)
    const row = page.locator(`[data-project-id="${projectId}"]`)
    await expect(row).toBeVisible({ timeout: 20000 })
    await row.click()

    await expect(page).toHaveURL(new RegExp(`/projects/${projectId}$`))
  })

  test('"All projects" link navigates to the projects list', async ({ page, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    await seedProject(apiURL, token, `e2e-allp-${Date.now()}`)

    await page.goto(`${baseURL}/`)
    const allProjects = byTestId(page, 'project-nav-all-projects-button')
    await expect(allProjects).toBeVisible({ timeout: 20000 })
    await allProjects.click()

    await expect(page).toHaveURL(/\/projects$/)
  })
})
