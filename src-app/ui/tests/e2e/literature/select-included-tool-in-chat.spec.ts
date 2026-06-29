import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  createModelViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'
import { seedAssistantWithToolResult } from '../chat/fixtures/mock-tool-result'

/**
 * E2E — the lit_search `select_included` tool result surfaces in chat.
 *
 * Audit gap: no E2E covered the chat-side rendering of a `select_included` tool
 * turn (only `literature_search` gets the special screening card; the others
 * render via the generic tool-result view). This seeds that tool_use/tool_result
 * assistant turn and asserts the answer renders. Only the SSE/tool boundary
 * is mocked; the chat rendering runs for real.
 */

test.describe('Literature — select_included tool result in chat', () => {
  test('a select_included tool turn renders the answer', async ({ page, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)

    const providerId = await createProviderViaAPI(apiURL, adminToken, 'OpenAI', 'openai')
    await assignProviderToAdministratorsGroup(apiURL, adminToken, providerId)
    await createModelViaAPI(apiURL, adminToken, providerId, undefined, undefined, 'openai')

    const answer =
      'After select_included: 12 unique records remain across the merged rounds.'

    await seedAssistantWithToolResult(page, baseURL, {
      toolName: 'select_included',
      serverId: 'lit_search.ziee.internal',
      resourceLinks: [],
      text: answer,
    })

    await expect(
      page.locator('[data-testid="chat-message"]').filter({ hasText: answer }),
    ).toBeVisible({ timeout: 15000 })
  })
})
