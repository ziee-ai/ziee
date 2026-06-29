import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  createModelViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'
import { seedAssistantWithToolResult } from './fixtures/mock-tool-result'

/**
 * E2E — the tool_result_mcp `get_tool_result` recall surfaces in chat.
 *
 * Audit gap (all-ba277ed997b6): the built-in `tool_result.ziee.internal`
 * server lets the model recall the exact stored content of an earlier tool
 * result mid-conversation (`get_tool_result(tool_use_id)` — used when an old
 * result was cleared/truncated to save context). The backend recall handler is
 * covered by `server/tests/tool_result_mcp/mod.rs`, but no E2E proved the
 * chat-side path: that a `get_tool_result` recall turn renders in the UI and
 * the recalled content surfaces in the assistant's answer.
 *
 * The model's *decision* to call the recall tool is the mocked boundary (the
 * SSE/tool stream), exactly as the sibling built-in-tool chat-render specs
 * (`semantic-search-tool-in-chat.spec.ts`, `literature/fetch-fulltext...`)
 * mock theirs. Everything downstream — the real chat message renderer mounting
 * the `get_tool_result` tool_use/tool_result blocks and the recalled answer —
 * runs for real.
 */

test.describe('Chat — tool_result recall (get_tool_result)', () => {
  test('a get_tool_result recall turn renders and surfaces the recalled content', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)
    const providerId = await createProviderViaAPI(apiURL, adminToken, 'OpenAI', 'openai')
    await assignProviderToAdministratorsGroup(apiURL, adminToken, providerId)
    await createModelViaAPI(apiURL, adminToken, providerId, undefined, undefined, 'openai')

    // A fact that lived only inside an EARLIER tool result; after that result
    // was cleared/truncated, the only way for it to reach the answer is via a
    // real `get_tool_result` recall of stored history.
    const beacon = `ZIEE_RECALL_BEACON_${Date.now()}`
    const recalledAnswer =
      `Recalling the earlier literature_search result via get_tool_result: ` +
      `its key finding was "${beacon}" — a 42% reduction in off-target effects.`

    // Seed an assistant turn whose tool_use is the built-in recall tool
    // (name `get_tool_result`, server `tool_result.ziee.internal`), followed by
    // the assistant's answer quoting the recalled fact.
    await seedAssistantWithToolResult(page, baseURL, {
      toolName: 'get_tool_result',
      serverId: 'tool_result.ziee.internal',
      resourceLinks: [],
      text: recalledAnswer,
    })

    // The recall tool turn rendered (the model invoked get_tool_result, not a
    // re-run of the original tool) — its name is shown in the tool-call block.
    await expect(page.getByText('get_tool_result').first()).toBeVisible({
      timeout: 15000,
    })

    // The recalled content surfaces in the assistant's answer.
    await expect(page.getByText(recalledAnswer)).toBeVisible({ timeout: 15000 })
    await expect(page.getByText(beacon, { exact: false }).first()).toBeVisible()
  })
})
