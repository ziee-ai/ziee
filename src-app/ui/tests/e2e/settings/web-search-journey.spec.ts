import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  createModelViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'
import { seedAssistantWithToolResult } from '../chat/fixtures/mock-tool-result'
import { byTestId } from '../testid.ts'

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
    await expect(byTestId(page, 'websearch-global-card')).toBeVisible({
      timeout: 30000,
    })

    // Web search is enabled deployment-wide by DEFAULT (web_search_settings
    // migration). Only toggle + save when it isn't already on — Save is
    // dirty-gated (disabled={!isDirty}), so clicking it without a real change
    // (i.e. when already enabled) just times out on a disabled button.
    const toggle = byTestId(page, 'websearch-global-enabled')
    if (!(await toggle.isChecked())) {
      await toggle.click()
      await byTestId(page, 'websearch-global-save').click()
      await expect(
        page.locator('[data-sonner-toast][data-type="success"]').first(),
      ).toBeVisible({ timeout: 10000 })
    }

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
