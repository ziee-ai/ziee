import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'
import { loginAsAdmin, getCurrentUserToken } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'
import { createBridgeToolModel, HAS_BRIDGE, BRIDGE_SKIP } from '../chat/helpers/agent-llm-helpers'
import { goToNewChatPage, selectModelInDropdown } from '../chat/helpers/chat-helpers'
import { ExternalMcpMockServer } from '../mcp/helpers/external-mcp-mock-server'

/**
 * TEST-181 / ITEM-50 — the EXTERNAL-server tool-approval card gives the human FULL
 * DISCLOSURE before they let data egress: the exact (untruncated) tool description
 * the model was given, the concrete arguments the model chose, and the destination
 * HOST the data is being sent to.
 *
 * A real bridge model is asked to call a tool on an EXTERNAL MCP server whose URL is
 * a NON-loopback local IP (so the backend's `resolve_dest_host()` returns
 * `Some(host)` — a loopback/built-in server resolves `None` and the card falls back
 * to the generic line). Manual approval surfaces the card; we assert its three
 * ITEM-50 disclosure elements render the real, verbatim data.
 *
 * Requires the agent-core chat path + a real LLM bridge, and a non-loopback IPv4
 * interface on the box (the mock throws otherwise). Skips cleanly when the bridge
 * is unset. --workers=1.
 */
test.describe('external MCP approval — full data-egress disclosure (ITEM-50)', () => {
  test.skip(!HAS_BRIDGE, BRIDGE_SKIP)
  test.setTimeout(240_000)

  let mock: ExternalMcpMockServer | undefined

  test.afterEach(async () => {
    await mock?.dispose()
  })

  test('external tool approval card shows the exact description, concrete args, and destination host', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getCurrentUserToken(page)
    const auth = { Authorization: `Bearer ${token}` }

    // A tool-capable bridge model so the model can actually choose to call the tool.
    const providerId = await createProviderViaAPI(apiURL, token, 'OpenAI', 'openai')
    await assignProviderToAdministratorsGroup(apiURL, token, providerId)
    await createBridgeToolModel(page, apiURL, token, providerId, 'External Disclosure Model')

    // An EXTERNAL MCP server advertised on a NON-loopback local IP → dest_host resolves.
    mock = await ExternalMcpMockServer.start()
    const created = await page.request.post(`${apiURL}/api/mcp/system-servers`, {
      headers: auth,
      data: {
        name: `ext_disclosure_${Date.now()}`,
        display_name: 'External Partner Server',
        description: 'external partner lookup server (non-loopback)',
        enabled: true,
        transport_type: 'http',
        url: mock.url(),
        timeout_seconds: 120,
        usage_mode: 'auto',
      },
    })
    expect(created.ok(), `create server failed: ${created.status()} ${await created.text()}`).toBeTruthy()
    const serverId = (await created.json()).id as string

    // Grant it to the default (Users) group so a new conversation attaches it.
    const groupsBody = await (await page.request.get(`${apiURL}/api/groups`, { headers: auth })).json()
    const groups = Array.isArray(groupsBody) ? groupsBody : (groupsBody.groups ?? [])
    const dg =
      groups.find((g: { is_default?: boolean }) => g.is_default) ??
      groups.find((g: { name: string }) => g.name === 'Users')
    await page.request.post(`${apiURL}/api/mcp/system-servers/${serverId}/groups`, {
      headers: auth,
      data: { group_ids: [dg.id] },
    })

    // MANUAL approval → the tool call must surface the approval card (never auto-run
    // an external server's tool).
    await page.request.put(`${apiURL}/api/mcp/defaults`, {
      headers: auth,
      data: {
        selected_servers: [{ server_id: serverId, tools: [] }],
        disabled_servers: [],
        approval_mode: 'manual_approve',
        auto_approved_tools: [],
      },
    })

    await goToNewChatPage(page, baseURL)
    await selectModelInDropdown(page, 'External Disclosure Model')

    const textarea = page.locator('textarea[placeholder*="Type your message"]')
    await textarea.fill(
      `Call the \`${ExternalMcpMockServer.TOOL_NAME}\` tool NOW with the argument ` +
        `query set to exactly "CRISPR delivery vectors". Do not answer from your own ` +
        `knowledge — you MUST call the tool.`,
    )
    await page.getByRole('button', { name: 'Send message' }).click()

    // The external-server approval card surfaces (manual_approve + external tool).
    const card = byTestId(page, 'mcp-tool-approval-card').first()
    await expect(card).toBeVisible({ timeout: 210_000 })

    // ITEM-50 disclosure #1 — the destination HOST the data egresses to (present
    // only for an external, non-loopback server).
    const destHost = card.locator('[data-testid="approval-dest-host"]')
    await expect(destHost).toBeVisible({ timeout: 15_000 })
    await expect(destHost).toContainText(mock.destHost())

    // ITEM-50 disclosure #2 — the FULL, EXACT tool description (never truncated).
    const desc = card.locator('[data-testid="approval-tool-description"]')
    await expect(desc).toBeVisible()
    await expect(desc).toHaveText(ExternalMcpMockServer.TOOL_DESCRIPTION)

    // ITEM-50 disclosure #3 — the concrete arguments the model chose (verbatim).
    const args = card.locator('[data-testid="approval-tool-args"]')
    await expect(args).toBeVisible()
    await expect(args).toContainText('CRISPR delivery vectors')
  })
})
