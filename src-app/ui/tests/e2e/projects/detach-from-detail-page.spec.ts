import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import { byTestId } from '../testid'

/**
 * E2E — detach a conversation from the PROJECT DETAIL PAGE conversations list.
 *
 * Existing specs cover detaching from the sidebar 3-dot menu and from the
 * /chats history. The detail page's `ProjectConversationsList` exposes its own
 * per-card "Remove from project" Popconfirm (`RemoveFromProjectButton`), which
 * was untested. This drives that affordance and asserts the DELETE fires and
 * the conversation leaves the project list.
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

async function seedConv(apiURL: string, token: string, title: string): Promise<string> {
  const res = await fetch(`${apiURL}/api/conversations`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${token}` },
    body: JSON.stringify({ title }),
  })
  if (!res.ok) throw new Error(`seed conv failed: ${res.status}`)
  return (await res.json()).id
}

async function attach(apiURL: string, token: string, projectId: string, conversationId: string) {
  const res = await fetch(
    `${apiURL}/api/projects/${projectId}/conversations/${conversationId}`,
    { method: 'POST', headers: { Authorization: `Bearer ${token}` } },
  )
  if (!res.ok) throw new Error(`attach failed: ${res.status}`)
}

test.describe('Projects — detach from the detail page conversations list', () => {
  test('"Remove from project" on a detail-page card detaches the conversation', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)

    const projectId = await seedProject(apiURL, token, `Detail Detach ${Date.now()}`)
    const convTitle = `Detail detach conv ${Date.now()}`
    const conversationId = await seedConv(apiURL, token, convTitle)
    await attach(apiURL, token, projectId, conversationId)

    let detachSeen = false
    page.on('request', req => {
      if (
        req.method() === 'DELETE' &&
        req.url().includes(`/api/projects/${projectId}/conversations/${conversationId}`)
      ) {
        detachSeen = true
      }
    })

    await page.goto(`${baseURL}/projects/${projectId}`)

    // The conversation appears in the project's detail-page list.
    const card = byTestId(page, `chat-conversation-card-${conversationId}`)
    await expect(card).toBeVisible({ timeout: 30000 })

    // Hover to reveal + click the per-card "Remove from project" button.
    await card.hover()
    await byTestId(card, 'project-conv-remove-trigger-button').click()

    // Confirm via the remove dialog.
    await expect(byTestId(page, 'project-conv-remove-dialog')).toBeVisible()
    await byTestId(page, 'project-conv-remove-confirm-button').click()

    // The DELETE fired and the card leaves the project list.
    await expect
      .poll(() => detachSeen, { timeout: 10000 })
      .toBe(true)
    await expect(
      byTestId(page, `chat-conversation-card-${conversationId}`),
    ).toHaveCount(0, { timeout: 10000 })
  })
})
