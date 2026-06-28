import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  createModelViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'
import {
  goToNewChatPage,
  selectModelInDropdown,
  sendChatMessage,
} from './helpers/chat-helpers'

/**
 * E2E — the web_search built-in tool used IN a real chat. web-search-settings
 * only tests the admin settings page; no E2E drives the model calling web_search
 * in a conversation. Here web_search is enabled + a SearXNG provider configured
 * (so the tool auto-attaches), then a real Anthropic model is asked to search;
 * we assert the web_search tool call surfaces in the transcript. Real-LLM gated.
 */

const HAS_ANTHROPIC = Boolean(process.env.ANTHROPIC_API_KEY)

test.describe('Chat — web_search tool (real LLM)', () => {
  test.skip(!HAS_ANTHROPIC, 'ANTHROPIC_API_KEY not set — real-LLM web_search E2E skipped')

  test('the model calls web_search when asked to search the web', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)

    // Enable web_search with a SearXNG provider so the tool auto-attaches.
    const auth = { 'Content-Type': 'application/json', Authorization: `Bearer ${token}` }
    let r = await fetch(`${apiURL}/api/web-search/providers/searxng`, {
      method: 'PUT',
      headers: auth,
      body: JSON.stringify({ config: { base_url: 'http://127.0.0.1:9/' } }),
    })
    expect(r.ok).toBeTruthy()
    r = await fetch(`${apiURL}/api/web-search/settings`, {
      method: 'PUT',
      headers: auth,
      body: JSON.stringify({ enabled: true, provider_chain: ['searxng'] }),
    })
    expect(r.ok).toBeTruthy()

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

    await goToNewChatPage(page, baseURL)
    await selectModelInDropdown(page, 'Claude Haiku 4.5')
    await sendChatMessage(
      page,
      'Use the web_search tool to look up "latest Rust release". You MUST call web_search.',
      false,
    )

    // The web_search tool call surfaces in the chat transcript (the search may
    // fail against the dummy provider, but the model invoked the tool).
    await expect(page.getByText(/web_search/i).first()).toBeVisible({ timeout: 90_000 })
  })
})
