import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  createModelViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'
import { goToNewChatPage, selectModelInDropdown, sendChatMessage } from '../chat/helpers/chat-helpers'
import { LiteratureMockServer, sampleLiteraturePayload } from './helpers/literature-mock-server'
import { byTestId } from '../testid'

/**
 * LLM-gated end-to-end for the literature screening panel via a REAL chat.
 *
 * The deterministic `screening-flow.spec.ts` seeds a `literature_search`
 * tool_result directly (mockChatTokenStream + mockGetMessages), so it never
 * exercises the production path where the MODEL decides to call the search
 * tool and the resulting structured_content flows through chat → tool_result
 * persistence → LiteratureToolResultCard → screening panel.
 *
 * This test drives that real path: a real Anthropic Haiku model is given a
 * tool-capable chat with a mock MCP server exposing a `literature_search`
 * tool. The model chooses to call it; the mock returns a typed
 * `structuredContent` LitStructured payload (mocking ONLY the upstream search
 * data — the model's tool-call decision, the backend structured_content
 * persistence, and the panel render are all real). The screening card
 * renders by tool NAME (`block.name === 'literature_search'`), so the mock's
 * tool stands in for the built-in lit_search server without live Europe PMC
 * network or the built-in being enabled.
 *
 * Skips cleanly without ANTHROPIC_API_KEY.
 */

const HAS_ANTHROPIC_KEY = Boolean(process.env.ANTHROPIC_API_KEY)

test.describe('Literature screening — real chat → tool → panel (real LLM + mock MCP)', () => {
  test.skip(!HAS_ANTHROPIC_KEY, 'ANTHROPIC_API_KEY not set — skipping LLM-gated tests')
  test.slow()

  let mock: LiteratureMockServer

  test.beforeEach(async ({ page, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(page)

    const providerId = await createProviderViaAPI(apiURL, token, 'Anthropic', 'anthropic')
    await assignProviderToAdministratorsGroup(apiURL, token, providerId)
    await createModelViaAPI(
      apiURL,
      token,
      providerId,
      'claude-haiku-4-5-20251001',
      'Claude Haiku 4.5',
      'anthropic',
    )

    mock = await LiteratureMockServer.start(sampleLiteraturePayload())

    // Register the mock as a system MCP server exposing `literature_search`.
    const created = await page.request.post(`${apiURL}/api/mcp/system-servers`, {
      headers: { Authorization: `Bearer ${token}` },
      data: {
        name: `mock_literature_${Date.now()}`,
        display_name: 'Mock Literature Search',
        description: 'Node mock exposing a literature_search tool with structured records',
        enabled: true,
        transport_type: 'http',
        url: mock.url(),
        timeout_seconds: 120,
        usage_mode: 'auto',
      },
    })
    const serverId: string = (await created.json()).id

    // Assign to the default group so the chat user can use it.
    const groupsRes = await page.request.get(`${apiURL}/api/groups`, {
      headers: { Authorization: `Bearer ${token}` },
    })
    const groupsBody = await groupsRes.json()
    const groups: Array<{ id: string; is_default?: boolean; name: string }> = Array.isArray(
      groupsBody,
    )
      ? groupsBody
      : (groupsBody.groups ?? [])
    const defaultGroup = groups.find(g => g.is_default) ?? groups.find(g => g.name === 'Users')
    if (defaultGroup) {
      await page.request.post(`${apiURL}/api/mcp/system-servers/${serverId}/groups`, {
        headers: { Authorization: `Bearer ${token}` },
        data: { group_ids: [defaultGroup.id] },
      })
    }

    // Auto-approve so the LLM-driven tool call doesn't block on approval.
    await page.request.put(`${apiURL}/api/mcp/defaults`, {
      headers: { Authorization: `Bearer ${token}` },
      data: {
        selected_servers: [{ server_id: serverId, tools: [] }],
        disabled_servers: [],
        approval_mode: 'auto_approve',
        auto_approved_tools: [],
      },
    })
  })

  test.afterEach(async () => {
    await mock?.dispose()
  })

  test('model calls literature_search → screening panel opens and screens a row', async ({
    page,
    testInfra,
  }) => {
    await goToNewChatPage(page, testInfra.baseURL)
    await selectModelInDropdown(page, 'Claude Haiku 4.5')

    await sendChatMessage(
      page,
      'Use the literature_search tool to find papers on CRISPR base editing off-target effects, ' +
        'then tell me how many records you found.',
      true,
    )

    // The inline LiteratureToolResultCard renders from the REAL persisted
    // tool_result block (name === 'literature_search') the model produced.
    const openBtn = byTestId(page, 'lit-tool-result-open-button')
    await expect(openBtn).toBeVisible({ timeout: 60000 })
    expect(mock.toolCallCount()).toBeGreaterThan(0)
    await openBtn.click()

    // The right-panel screening workbench opens with the real records.
    await expect(byTestId(page, 'lit-screening-panel')).toBeVisible({ timeout: 15000 })
    await expect(byTestId(page, 'lit-screening-records-list')).toContainText(
      'Base editing reduces off-target effects',
      { timeout: 10000 },
    )

    // Exercise one real screening interaction: bulk-include both rows and
    // watch the PRISMA Included count update through the production panel.
    await byTestId(page, 'lit-screening-select-all-checkbox').click()
    await byTestId(page, 'lit-screening-bulk-include-button').click()
    await expect(byTestId(page, 'lit-screening-tag-included')).toContainText('2', {
      timeout: 10000,
    })
  })
})

async function getAdminToken(page: import('@playwright/test').Page): Promise<string> {
  const authData = await page.evaluate(() => localStorage.getItem('auth-storage'))
  return JSON.parse(authData!).state.token
}
