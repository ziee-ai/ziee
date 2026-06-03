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

    // 2. Create a conversation inside the project via two API calls:
    //    chat creates UNFILED, then project attaches it. After the
    //    chat↔project decoupling, chat's POST /conversations no
    //    longer accepts project_id in the body — project membership
    //    is a separate concern owned by the project module's attach
    //    endpoint. This mirrors the production frontend flow (the
    //    project chat extension's afterCreateConversation hook).
    const token = await getAdminToken(baseURL)
    await page.locator('.ant-card', { hasText: 'Ephemeral Project' }).click()
    await page.waitForURL(/\/projects\/[0-9a-f-]+$/)
    const projectId = new URL(page.url()).pathname.split('/').pop()!
    const convResp = await page.evaluate(
      async ({ apiBase, pid, t }: { apiBase: string; pid: string; t: string }) => {
        // Step 1: chat creates unfiled conversation.
        const createRes = await fetch(`${apiBase}/api/conversations`, {
          method: 'POST',
          headers: {
            'Content-Type': 'application/json',
            Authorization: `Bearer ${t}`,
          },
          body: JSON.stringify({ title: 'Orphan Soon' }),
        })
        const created = await createRes.json()
        // Step 2: attach to project.
        const attachRes = await fetch(
          `${apiBase}/api/projects/${pid}/conversations/${created.id}`,
          {
            method: 'POST',
            headers: { Authorization: `Bearer ${t}` },
          },
        )
        if (!attachRes.ok) {
          throw new Error(`Attach failed: ${attachRes.status}`)
        }
        return created
      },
      { apiBase: baseURL, pid: projectId, t: token },
    )
    expect(convResp).toHaveProperty('id')

    // 3. Delete the project from the list-page card via the inline
    //    Delete icon button + Popconfirm. The round-3 ProjectCard
    //    rewrite replaced the Dropdown menu with inline icon buttons.
    await goToProjectsPage(page, baseURL)
    await clickCardAction(page, 'Ephemeral Project', 'Delete')
    await confirmDeletePopconfirm(page)

    // 4. The conversation still exists AND is now unfiled. Schema
    //    move (migration 73): project membership lives in the
    //    `project_conversations` join table; deleting the project
    //    cascades that row away, leaving the conversation orphaned.
    //    Verify via GET /conversations (still 200) + GET
    //    /projects/by-conversation (404 = unfiled).
    const after = await page.evaluate(
      async ({ apiBase, t, cid }: { apiBase: string; t: string; cid: string }) => {
        const convResp = await fetch(`${apiBase}/api/conversations/${cid}`, {
          headers: { Authorization: `Bearer ${t}` },
        })
        const projectResp = await fetch(
          `${apiBase}/api/projects/by-conversation/${cid}`,
          { headers: { Authorization: `Bearer ${t}` } },
        )
        return {
          convStatus: convResp.status,
          projectLookupStatus: projectResp.status,
        }
      },
      { apiBase: baseURL, t: token, cid: convResp.id as string },
    )
    expect(after.convStatus).toBe(200)
    expect(after.projectLookupStatus).toBe(404)
  })
})
