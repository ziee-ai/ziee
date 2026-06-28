import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'

/**
 * E2E — moving a conversation from one project to another.
 *
 * Audit gap: project specs cover create-in-project + remove-from-project, but
 * not MOVING a conversation between two projects. Re-attaching a conversation
 * to project B (it is currently in A) must move it: B's detail page lists it
 * and A's no longer does.
 */

async function createProject(apiURL: string, token: string, name: string) {
  const res = await fetch(`${apiURL}/api/projects`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${token}` },
    body: JSON.stringify({ name }),
  })
  if (!res.ok) throw new Error(`create project: ${res.status}`)
  return (await res.json()).id as string
}

async function attach(apiURL: string, token: string, projectId: string, convId: string) {
  const res = await fetch(
    `${apiURL}/api/projects/${projectId}/conversations/${convId}`,
    { method: 'POST', headers: { Authorization: `Bearer ${token}` } },
  )
  if (!res.ok) throw new Error(`attach: ${res.status}`)
}

test.describe('Projects — move conversation between projects', () => {
  test('re-attaching to project B moves the conversation off project A', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)

    const ts = Date.now().toString(36)
    const projectA = await createProject(apiURL, token, `Proj A ${ts}`)
    const projectB = await createProject(apiURL, token, `Proj B ${ts}`)
    const title = `Movable Conv ${ts}`
    const convRes = await fetch(`${apiURL}/api/conversations`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${token}` },
      body: JSON.stringify({ title }),
    })
    const convId = (await convRes.json()).id as string
    await attach(apiURL, token, projectA, convId)

    const convSection = () => page.locator('[data-test-section="conversations"]')

    // Initially in A.
    await page.goto(`${baseURL}/projects/${projectA}`)
    await expect(convSection().getByText(title)).toBeVisible({ timeout: 30000 })

    // MOVE: re-attach to B.
    await attach(apiURL, token, projectB, convId)

    // Now in B…
    await page.goto(`${baseURL}/projects/${projectB}`)
    await expect(convSection().getByText(title)).toBeVisible({ timeout: 30000 })

    // …and no longer in A.
    await page.goto(`${baseURL}/projects/${projectA}`)
    await expect(convSection()).toBeVisible({ timeout: 30000 })
    await expect(convSection().getByText(title)).toHaveCount(0)
  })
})
