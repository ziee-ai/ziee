import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  createModelViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'
import { seedAssistantWithToolResult } from '../09-chat/fixtures/mock-tool-result'

/**
 * E2E — the full web-search journey: an admin CONFIGURES web search on the
 * settings page, then a chat surfaces a `web_search` tool result.
 *
 * Audit gap: no test connected web-search settings configuration to chat
 * behaviour. The settings spec (`web-search-settings.spec.ts`) stops at the
 * admin form; the chat specs never exercise a web_search tool_result. This
 * stitches the two halves into one user journey.
 *
 * Phase 1 (real backend): enable web search on /settings/web-search and Save.
 * Phase 2 (deterministic): seed an assistant turn whose `web_search`
 * tool_result feeds a text answer, and assert the answer renders in chat.
 * Only the chat SSE/messages HTTP boundary is mocked.
 */
test.describe('Web search — configure then chat journey', () => {
  test('enable web search → chat renders a web_search-backed answer', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)

    // ── Phase 1: configure web search on the real settings backend ──
    await page.goto(`${baseURL}/settings/web-search`)
    await expect(
      page.getByRole('heading', { name: 'Web Search' }),
    ).toBeVisible({ timeout: 30000 })

    const toggle = page.getByRole('switch').first()
    if (!(await toggle.isChecked())) {
      await toggle.click()
    }
    await page
      .locator('form')
      .filter({ has: page.getByLabel('Max results per search') })
      .getByRole('button', { name: 'Save' })
      .click()
    await expect(page.getByText('Web search settings saved')).toBeVisible({
      timeout: 10000,
    })

    // ── Phase 2: seed a chat turn backed by a web_search tool_result ──
    const token = await page.evaluate(() =>
      JSON.parse(localStorage.getItem('auth-storage')!).state.token,
    )
    const providerId = await createProviderViaAPI(apiURL, token, 'OpenAI', 'openai')
    await assignProviderToAdministratorsGroup(apiURL, token, providerId)
    await createModelViaAPI(apiURL, token, providerId, undefined, undefined, 'openai')

    const ANSWER = 'Per web search, the capital of France is Paris.'
    const { assistantMessageId } = await seedAssistantWithToolResult(
      page,
      baseURL,
      { toolName: 'web_search', serverId: 'web-search-test', resourceLinks: [], text: ANSWER },
    )

    const bubble = page
      .locator(`[data-testid="chat-message"][data-message-id="${assistantMessageId}"]`)
      .first()
    await expect(bubble).toBeVisible({ timeout: 15000 })
    // The model's web-search-derived answer renders in the bubble.
    await expect(bubble.getByText(ANSWER)).toBeVisible({ timeout: 10000 })
  })
})
