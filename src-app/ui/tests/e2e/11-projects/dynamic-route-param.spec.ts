import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'

/**
 * E2E — dynamic route segment (`:projectId`) param resolution.
 *
 * Routes declare path params (router/types.ts); the project detail route
 * `/projects/:projectId` must extract the id from the URL and render THAT
 * project. This seeds two projects and navigates to each by id, asserting the
 * page renders the matching project's name — proving the dynamic segment
 * resolves distinctly per id.
 */

async function seedProject(apiURL: string, token: string, name: string): Promise<string> {
  const res = await fetch(`${apiURL}/api/projects`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${token}` },
    body: JSON.stringify({ name }),
  })
  if (!res.ok) throw new Error(`seed project failed: ${res.status}`)
  return (await res.json()).id
}

test.describe('Router — dynamic route segment', () => {
  test('/projects/:projectId renders the project matching the URL id', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)

    const nameA = `RouteParam A ${Date.now()}`
    const nameB = `RouteParam B ${Date.now()}`
    const idA = await seedProject(apiURL, token, nameA)
    const idB = await seedProject(apiURL, token, nameB)

    // Navigate by id A → header shows A's name (not B's).
    await page.goto(`${baseURL}/projects/${idA}`)
    await expect(
      page.getByRole('heading', { level: 4, name: nameA }),
    ).toBeVisible({ timeout: 30000 })
    await expect(
      page.getByRole('heading', { level: 4, name: nameB }),
    ).toHaveCount(0)

    // Switch the dynamic segment to id B → header now shows B's name.
    await page.goto(`${baseURL}/projects/${idB}`)
    await expect(
      page.getByRole('heading', { level: 4, name: nameB }),
    ).toBeVisible({ timeout: 30000 })
  })
})
