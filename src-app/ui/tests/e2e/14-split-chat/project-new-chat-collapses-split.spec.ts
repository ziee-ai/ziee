import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  createModelViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'

/**
 * Split-chat E2E — starting a chat from a PROJECT also collapses the split.
 *
 * The sibling spec (`new-chat-collapses-split.spec.ts`) covers the sidebar
 * "New Chat" path. This one covers the surface that reproduced the identical
 * bug through a different door: `ProjectDetailPage` is that page's structural
 * twin — same `Stores.Chat.reset()`, same `conversation.created` subscription,
 * same navigate-on-create — and its target route
 * `/projects/:id/chat/:conversationId` renders the SAME `ConversationPage`. So
 * with a split still open in the store, its URL→workspace reconcile took the
 * "auto while split" branch and wedged the brand-new project conversation into
 * the old split.
 *
 * Without this spec the fix on that surface has no coverage at any tier:
 * deleting the `Stores.SplitView.reset()` from ProjectDetailPage's created
 * handler leaves the unit test, the sidebar e2e and every gate green.
 */
test.describe('Split chat — a project chat also collapses the split', () => {
  test('TEST-10: 2-pane split → project composer → send lands on a SINGLE pane', async ({
    page,
    testInfra,
  }) => {
    test.setTimeout(120000)
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)

    const providerId = await createProviderViaAPI(apiURL, token, 'Bridge', 'openai')
    await assignProviderToAdministratorsGroup(apiURL, token, providerId)
    await createModelViaAPI(apiURL, token, providerId, undefined, undefined, 'openai')

    const mkConv = async (title: string): Promise<string> => {
      const res = await page.request.post(`${apiURL}/api/conversations`, {
        headers: { Authorization: `Bearer ${token}` },
        data: { title },
      })
      expect(res.status()).toBeLessThan(300)
      return (await res.json()).id as string
    }
    const convA = await mkConv('Project Collapse Alpha')
    const convB = await mkConv('Project Collapse Bravo')

    const projectRes = await page.request.post(`${apiURL}/api/projects`, {
      headers: { Authorization: `Bearer ${token}` },
      data: { name: 'Collapse Project', description: 'split-collapse coverage' },
    })
    expect(projectRes.status()).toBeLessThan(300)
    const projectId = (await projectRes.json()).id as string

    // ── Build the same starting state: two conversations side by side.
    await page.goto(`${baseURL}/chat/${convA}`)
    await page.waitForLoadState('load')
    await byTestId(page, 'chat-split-btn').click()
    const pane1 = byTestId(page, 'chat-pane-1')
    await expect(pane1).toBeVisible({ timeout: 15000 })
    await pane1.getByTestId(`conversation-picker-item-${convB}`).click()
    await expect(pane1.getByTestId('conversation-picker-pane')).toHaveCount(0)
    await expect(byTestId(page, 'split-chat-view')).toBeVisible()

    // ── Now start a chat from the PROJECT page rather than the sidebar.
    await page.goto(`${baseURL}/projects/${projectId}`)
    const input = page.locator('textarea[placeholder*="Type your message"]')
    await expect(input).toBeVisible({ timeout: 30000 })
    await input.click()
    await input.fill('Reply with exactly the single word: PONG')
    const send = page.getByRole('button', { name: 'Send message' })
    await expect(send).toBeEnabled({ timeout: 30000 })
    await send.click()

    // It lands on the project-namespaced conversation route — which renders the
    // same ConversationPage — as a SINGLE pane, not the resurrected split.
    await page.waitForURL(/\/projects\/[0-9a-f-]{36}\/chat\/[0-9a-f-]{36}$/i, {
      timeout: 30000,
    })
    await expect(page.locator('[data-role="user"]')).toBeVisible({ timeout: 30000 })
    await expect(byTestId(page, 'split-chat-view')).toHaveCount(0)
    await expect(byTestId(page, 'chat-pane-1')).toHaveCount(0)
    await expect(
      page.locator('textarea[placeholder*="Type your message"]'),
    ).toHaveCount(1)

    // ...and it is a NEW conversation, not one of the two that were open.
    const newId = page.url().split('/').pop()!
    expect(newId).not.toBe(convA)
    expect(newId).not.toBe(convB)
  })
})
