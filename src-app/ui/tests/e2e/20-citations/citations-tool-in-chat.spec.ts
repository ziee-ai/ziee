import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  createModelViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'
import { seedAssistantWithToolResult } from '../09-chat/fixtures/mock-tool-result'

/**
 * E2E — the citations MCP tool flow surfaces in chat.
 *
 * Audit gap: server/tests/citations/real_llm.rs covers the model→tool half of
 * the flow (a tool-capable model invokes the citations tools, data from
 * loopback mocks) but there was no E2E covering the CHAT-side rendering of a
 * citations tool turn. This seeds a `verify_citations` tool_use/tool_result
 * assistant turn and asserts the verified-citation answer renders in chat.
 * Only the SSE/tool boundary is mocked; the chat rendering runs for real.
 */

test.describe('Citations — tool result renders in chat', () => {
  test('a verify_citations tool turn renders the verified answer', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)

    // A provider + model so the chat page bootstraps cleanly.
    const providerId = await createProviderViaAPI(apiURL, adminToken, 'OpenAI', 'openai')
    await assignProviderToAdministratorsGroup(apiURL, adminToken, providerId)
    await createModelViaAPI(apiURL, adminToken, providerId, undefined, undefined, 'openai')

    const verified =
      'Verified: Smith et al. (2021), doi:10.1038/nature12373 is a real record.'

    await seedAssistantWithToolResult(page, baseURL, {
      toolName: 'verify_citations',
      serverId: 'citations.ziee.internal',
      resourceLinks: [],
      text: verified,
    })

    // The assistant turn (backed by the citations tool_result) renders.
    await expect(page.locator('[data-role="assistant"]')).toContainText(verified, {
      timeout: 15000,
    })
  })
})
