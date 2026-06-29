import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  createModelViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'
import { seedAssistantWithToolResult } from './fixtures/mock-tool-result'

/**
 * E2E — the files_mcp `semantic_search` tool result surfaces in chat.
 *
 * Audit gap: semantic_search (file_rag retrieval) is a model-driven MCP tool;
 * backend retrieval is covered server-side but no E2E covered the chat-side
 * rendering of a semantic_search tool turn. This seeds that tool_use/tool_result
 * assistant turn and asserts the retrieved-snippet answer renders in chat.
 * Only the SSE/tool boundary is mocked.
 */

test.describe('Chat — semantic_search tool result', () => {
  test('a semantic_search tool turn renders the retrieved answer', async ({
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
      'From your documents (semantic_search): the deployment guide lists 3 prerequisites.'

    await seedAssistantWithToolResult(page, baseURL, {
      toolName: 'semantic_search',
      serverId: 'files.ziee.internal',
      resourceLinks: [],
      text: answer,
    })

    await expect(page.getByText(answer)).toBeVisible({ timeout: 15000 })
  })
})
