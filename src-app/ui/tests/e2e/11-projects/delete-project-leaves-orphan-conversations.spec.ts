import { test, expect } from '../../fixtures/test-context'
import { getAdminToken, loginAsAdmin } from '../../common/auth-helpers'
import {
  clickCardAction,
  confirmDeletePopconfirm,
  fillProjectForm,
  goToProjectsPage,
  openCreateProjectDrawer,
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
    //    The token lives under the persisted `auth-storage` Zustand
    //    key, not `access_token` — fetch a fresh token via the helper
    //    so we don't depend on the FE storage shape.
    const token = await getAdminToken(baseURL)
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

    // 3. Delete the project from the list-page card via the inline
    //    Delete icon button + Popconfirm. The round-3 ProjectCard
    //    rewrite replaced the Dropdown menu with inline icon buttons.
    await goToProjectsPage(page, baseURL)
    await clickCardAction(page, 'Ephemeral Project', 'Delete')
    await confirmDeletePopconfirm(page)

    // 4. The conversation still exists at the API level (project_id = NULL).
    const after = await page.evaluate(
      async ({ apiBase, t, cid }: { apiBase: string; t: string; cid: string }) => {
        const r = await fetch(`${apiBase}/api/conversations/${cid}`, {
          headers: { Authorization: `Bearer ${t}` },
        })
        return {
          status: r.status,
          body: r.status === 200 ? await r.json() : null,
        }
      },
      { apiBase: baseURL, t: token, cid: convResp.id as string },
    )
    expect(after.status).toBe(200)
    expect(after.body.project_id).toBeNull()
  })
})
