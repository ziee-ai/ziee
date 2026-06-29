import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  createModelViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'
import { seedAssistantWithToolResult } from '../chat/fixtures/mock-tool-result'

/**
 * E2E — the lit_search `fetch_paper_fulltext` tool flow surfaces in chat.
 *
 * Audit gap: full-text fetching is a model-driven MCP tool (there is no
 * user-facing "fetch" button in the screening panel), and the backend tests
 * (lit_search/fulltext_test.rs) cover the resolver/cache side — but no E2E
 * covered the CHAT-side rendering of a fetch_paper_fulltext tool turn. This
 * seeds that tool_use/tool_result assistant turn and asserts the fetched
 * full-text answer renders. Only the SSE/tool boundary is mocked.
 */

test.describe('Literature — fetch_paper_fulltext tool result in chat', () => {
  test('a fetch_paper_fulltext tool turn renders the fetched answer', async ({
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
      'Full text fetched: the paper PMC123456 reports a 37% reduction in off-target effects.'

    await seedAssistantWithToolResult(page, baseURL, {
      toolName: 'fetch_paper_fulltext',
      serverId: 'lit_search.ziee.internal',
      resourceLinks: [],
      text: answer,
    })

    await expect(
      page.locator('[data-testid="chat-message"]').filter({ hasText: answer }),
    ).toBeVisible({ timeout: 15000 })
  })
})
