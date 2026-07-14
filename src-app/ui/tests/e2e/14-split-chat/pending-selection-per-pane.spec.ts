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
 * pending KB + MCP selection. Two complementary isolation proofs:
 *   • KB (attach-isolation): a KB is NOT auto-seeded, so attaching one in new-chat
 *     pane A shows its chip in pane A ONLY.
 *   • MCP (deselect-isolation): an admin-enabled server DOES auto-seed into EACH
 *     new pane's OWN pending config (per-pane McpInitializer), so it shows in BOTH;
 *     removing it from pane A's chip edits only pane A's pending buffer — gone from
 *     A, still in B.
 * Before ITEM-51 both panes shared one global pending buffer (`__pending__`), so a
 * pending selection/removal in one pane leaked into the other. No LLM.
 */
test.describe('Split chat — per-pane PENDING selection isolation (new chats)', () => {
  test.describe.configure({ retries: 1 })

  test('a pending KB attach + MCP removal in new-chat pane A stay isolated to pane A', async ({
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

    // ── MCP leg (deselect-isolation): the admin-enabled server auto-seeds into
    // EACH new pane's OWN pending config (per-pane McpInitializer), so it shows in
    // BOTH new panes independently. ──
    await expect(paneA.getByTestId(`mcp-chip-${serverId}`)).toBeVisible({ timeout: 15000 })
    await expect(paneB.getByTestId(`mcp-chip-${serverId}`)).toBeVisible({ timeout: 15000 })

    // Remove it from pane A ONLY via its chip × (deselectServerForConversation on
    // pane A's OWN pending buffer) — gone from A, STILL in B.
    await paneA
      .getByTestId(`mcp-chip-${serverId}`)
      .getByRole('button', { name: 'Remove' })
      .click()
    await expect(paneA.getByTestId(`mcp-chip-${serverId}`)).toHaveCount(0, { timeout: 10000 })
    await expect(paneB.getByTestId(`mcp-chip-${serverId}`)).toBeVisible()

    // ── KB leg (attach-isolation): a KB is NOT auto-seeded; attaching one in pane
    // A's + menu shows its chip in pane A's pending ONLY. ──
    await paneA.getByTestId('chat-input-add-btn').click()
    await byTestId(page, 'kb-menu-trigger').click()
    await byTestId(page, `kb-option-${kb.id}`).click()
    await expect(paneA.getByTestId(`kb-chip-${kb.id}`)).toBeVisible({ timeout: 15000 })
    await expect(paneB.getByTestId(`kb-chip-${kb.id}`)).toHaveCount(0)

    // Focusing pane B does not surface pane A's pending state, nor resurrect the
    // MCP server pane A removed (per-pane, not focus-following).
    await paneB.click({ position: { x: 200, y: 80 } })
    await expect(paneB.getByTestId(`kb-chip-${kb.id}`)).toHaveCount(0)
    await expect(paneA.getByTestId(`kb-chip-${kb.id}`)).toBeVisible()
    await expect(paneA.getByTestId(`mcp-chip-${serverId}`)).toHaveCount(0) // removed in A
    await expect(paneB.getByTestId(`mcp-chip-${serverId}`)).toBeVisible() // B keeps its own
  })
})
