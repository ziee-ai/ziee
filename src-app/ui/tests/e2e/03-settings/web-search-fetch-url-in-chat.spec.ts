import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  createModelViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'
import { seedAssistantWithToolResult } from '../09-chat/fixtures/mock-tool-result'

/**
 * E2E — the web_search `fetch_url` tool result surfaces in chat.
 *
 * Audit gap: fetch_url (page fetch + SSRF guard + markdown extraction) is a
 * model-driven MCP tool with no panel UI; the SSRF/extraction is covered by
 * backend tests (web_search/mcp_test.rs) but no E2E covered the chat-side
 * rendering of a fetch_url tool turn. This seeds that tool_use/tool_result
 * assistant turn and asserts the fetched-page answer renders in chat.
 */

test.describe('Web search — fetch_url tool result in chat', () => {
  test('a fetch_url tool turn renders the fetched-page answer', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)

    const providerId = await createProviderViaAPI(apiURL, adminToken, 'OpenAI', 'openai')
    await assignProviderToAdministratorsGroup(apiURL, adminToken, providerId)
    await createModelViaAPI(apiURL, adminToken, providerId, undefined, undefined, 'openai')

    const answer =
      'From the fetched page: the project README documents a 3-step install.'

    await seedAssistantWithToolResult(page, baseURL, {
      toolName: 'fetch_url',
      serverId: 'web_search.ziee.internal',
      resourceLinks: [],
      text: answer,
    })

    await expect(page.getByText(answer)).toBeVisible({ timeout: 15000 })
  })
})
