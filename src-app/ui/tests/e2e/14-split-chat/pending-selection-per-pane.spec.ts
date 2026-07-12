import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  createModelViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'

/**
 * Split-chat E2E — per-PANE PENDING selection isolation (TEST-78, ITEM-51). Two
 * split panes each composing a NEW (not-yet-created) chat must each hold their OWN
 * pending KB + MCP selection: a KB attached / an MCP server enabled in new-chat
 * pane A must NOT appear in new-chat pane B. Before ITEM-51 both panes shared one
 * global pending buffer (`__pending__`), so pane A's pending selection leaked into
 * pane B; now the pending buffer is keyed per pane. No LLM (attach + assert chip).
 */
test.describe('Split chat — per-pane PENDING selection isolation (new chats)', () => {
  test.describe.configure({ retries: 1 })

  test('a pending KB + MCP selection in new-chat pane A does not appear in new-chat pane B', async ({
    page,
    testInfra,
  }) => {
    test.setTimeout(120000)
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    const auth = { 'Content-Type': 'application/json', Authorization: `Bearer ${token}` }
    const providerId = await createProviderViaAPI(apiURL, token, 'OpenAI', 'openai')
    await assignProviderToAdministratorsGroup(apiURL, token, providerId)
    await createModelViaAPI(apiURL, token, providerId, undefined, undefined, 'openai')

    const stamp = Date.now()
    const kb = await (
      await page.request.post(`${apiURL}/api/knowledge-bases`, {
        headers: auth,
        data: { name: `Pending KB ${stamp}` },
      })
    ).json()
    const srv = await (
      await page.request.post(`${apiURL}/api/mcp/servers`, {
        headers: auth,
        data: {
          name: `pending-mcp-${stamp}`,
          display_name: `Pending MCP ${stamp}`,
          transport_type: 'http',
          url: 'https://pending-mcp.example.invalid/mcp',
          enabled: true,
        },
      })
    ).json()
    const serverId = srv.id as string
    // A saved conversation to ANCHOR the split (the split button lives in a
    // ConversationPane header; the bare `/chat` new-chat page has no such header).
    const convAnchor = await (
      await page.request.post(`${apiURL}/api/conversations`, {
        headers: auth,
        data: { title: `Anchor ${stamp}` },
      })
    ).json()

    // Pane 0 = the anchor conversation. Then add TWO panes that each start their
    // OWN new chat via the picker — so panes 1 AND 2 are both NEW (pre-mint) chats,
    // the exact two-new-chat case FB-11 is about.
    await page.goto(`${baseURL}/chat/${convAnchor.id}`)
    await page.waitForLoadState('load')
    const pane0 = byTestId(page, 'chat-pane-0')
    const paneA = byTestId(page, 'chat-pane-1') // new chat #1
    const paneB = byTestId(page, 'chat-pane-2') // new chat #2

    await byTestId(page, 'chat-split-btn').click()
    await expect(paneA).toBeVisible({ timeout: 15000 })
    await paneA.getByTestId('pane-start-new-chat').click()
    await expect(paneA.getByTestId('pane-new-chat-greeting')).toBeVisible({ timeout: 15000 })

    // Add a THIRD pane (a second picker) via pane 0's own split button, then start
    // its own new chat.
    await pane0.getByTestId('chat-split-btn').click()
    await expect(paneB).toBeVisible({ timeout: 15000 })
    await paneB.getByTestId('pane-start-new-chat').click()
    await expect(paneB.getByTestId('pane-new-chat-greeting')).toBeVisible({ timeout: 15000 })

    // ── MCP leg first: enable a server in new-chat pane A (its OWN pending config).
    // (The MCP config modal cleanly closes the "+" dropdown, leaving a fresh state
    // for the KB leg's own dropdown afterwards.) ──
    await paneA.getByTestId('chat-input-add-btn').click()
    await byTestId(page, 'chat-mcp-menu-item').first().click()
    await expect(byTestId(page, 'mcp-config-modal')).toBeVisible({ timeout: 10000 })
    const toggle = byTestId(page, `mcp-config-server-switch-${serverId}`)
    if ((await toggle.getAttribute('aria-checked')) !== 'true') await toggle.click()
    await expect(toggle).toHaveAttribute('aria-checked', 'true', { timeout: 5000 })
    await byTestId(page, 'mcp-config-close-btn').click()

    // MCP chip in pane A's pending config ONLY — pane B's own pending has no server.
    await expect(paneA.getByTestId(`mcp-chip-${serverId}`)).toBeVisible({ timeout: 15000 })
    await expect(paneB.getByTestId(`mcp-chip-${serverId}`)).toHaveCount(0)

    // ── KB leg: attach a KB in new-chat pane A (its OWN pending buffer) ──
    await paneA.getByTestId('chat-input-add-btn').click()
    await byTestId(page, 'kb-menu-trigger').click()
    await byTestId(page, `kb-option-${kb.id}`).click()

    // Chip shows in pane A's pending selection ONLY — pane B's own pending is empty.
    await expect(paneA.getByTestId(`kb-chip-${kb.id}`)).toBeVisible({ timeout: 15000 })
    await expect(paneB.getByTestId(`kb-chip-${kb.id}`)).toHaveCount(0)

    // Focusing pane B does not surface pane A's pending selections (per-pane, not focus).
    await paneB.click({ position: { x: 200, y: 80 } })
    await expect(paneB.getByTestId(`kb-chip-${kb.id}`)).toHaveCount(0)
    await expect(paneB.getByTestId(`mcp-chip-${serverId}`)).toHaveCount(0)
    await expect(paneA.getByTestId(`kb-chip-${kb.id}`)).toBeVisible()
    await expect(paneA.getByTestId(`mcp-chip-${serverId}`)).toBeVisible()
  })
})
