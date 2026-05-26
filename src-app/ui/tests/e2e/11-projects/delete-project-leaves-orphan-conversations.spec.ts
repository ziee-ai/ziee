import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import {
  clickCardMenuItem,
  fillProjectForm,
  goToProjectsPage,
  openCreateProjectDrawer,
  openProjectCardMenu,
  submitProjectForm,
} from './helpers/project-helpers'

/**
 * The locked semantics: deleting a project preserves its conversations
 * (project_id → NULL via ON DELETE SET NULL). The conversations remain
 * in the user's "Recent (unfiled)" list and no longer receive project
 * knowledge on future sends.
 */
test.describe('Projects - delete preserves orphan conversations', () => {
  test('project delete leaves its conversations visible as unfiled', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await goToProjectsPage(page, baseURL)

    // 1. Seed a project.
    await openCreateProjectDrawer(page)
    await fillProjectForm(page, { name: 'Ephemeral Project' })
    await submitProjectForm(page)

    // 2. Create a conversation inside the project via the API (drives
    //    the project_id snapshot path tested at the backend level).
    const token = await page.evaluate(() =>
      localStorage.getItem('access_token'),
    )
    await page.locator('.ant-card', { hasText: 'Ephemeral Project' }).click()
    await page.waitForURL(/\/projects\/[0-9a-f-]+$/)
    const projectId = new URL(page.url()).pathname.split('/').pop()!
    const convResp = await page.evaluate(
      async ([apiBase, pid, t]) => {
        const r = await fetch(`${apiBase}/api/conversations`, {
          method: 'POST',
          headers: {
            'Content-Type': 'application/json',
            Authorization: `Bearer ${t}`,
          },
          body: JSON.stringify({ project_id: pid, title: 'Orphan Soon' }),
        })
        return await r.json()
      },
      [baseURL, projectId, token],
    )
    expect(convResp).toHaveProperty('id')

    // 3. Delete the project from the list-page card menu.
    await goToProjectsPage(page, baseURL)
    await openProjectCardMenu(page, 'Ephemeral Project')
    await clickCardMenuItem(page, 'Delete')

    // 4. The conversation still exists at the API level (project_id = NULL).
    const after = await page.evaluate(
      async ([apiBase, t, cid]) => {
        const r = await fetch(`${apiBase}/api/conversations/${cid}`, {
          headers: { Authorization: `Bearer ${t}` },
        })
        return { status: r.status, body: r.status === 200 ? await r.json() : null }
      },
      [baseURL, token, convResp.id],
    )
    expect(after.status).toBe(200)
    expect(after.body.project_id).toBeNull()
  })
})
