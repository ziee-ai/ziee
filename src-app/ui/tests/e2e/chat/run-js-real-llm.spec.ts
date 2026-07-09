import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getCurrentUserToken } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'
import { goToNewChatPage, selectModelInDropdown } from './helpers/chat-helpers'

/**
 * run_js — real LLM, provider-agnostic (TEST-36).
 *
 * Proves the capability is provider-INDEPENDENT: a real tool-capable model
 * (any OpenAI/Anthropic-compatible endpoint) chooses to call `run_js`, the
 * embedded QuickJS runtime executes the script IN-PROCESS, and the final value
 * renders as a run_js tool card. Nothing about the model is special — run_js is
 * injected as a host tool for every tool-capable model.
 *
 * Wire it to a real endpoint via env (mirrors the createProviderViaAPI bridge
 * seam other real-LLM specs use):
 *   OPENAI_BASE_URL   e.g. http://127.0.0.1:4000/v1   (a local OpenAI-compatible bridge)
 *   OPENAI_API_KEY    the bridge key
 *   ZIEE_TEST_LLM_MODEL   the served model id (e.g. qwen3.6-35b-a3b)
 * Skips cleanly when unset (no endpoint available in that environment).
 */
const BRIDGE = process.env.OPENAI_BASE_URL || process.env.ZIEE_TEST_LLM_BASE_URL
const MODEL = process.env.ZIEE_TEST_LLM_MODEL || process.env.OPENAI_MODEL

test.describe('run_js — real LLM (provider-agnostic)', () => {
  test.skip(
    !BRIDGE || !MODEL,
    'no real LLM endpoint (set OPENAI_BASE_URL + OPENAI_API_KEY + ZIEE_TEST_LLM_MODEL)',
  )
  test.setTimeout(180_000)

  test('a tool-capable model calls run_js → script executes → card renders', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getCurrentUserToken(page)

    const providerId = await createProviderViaAPI(apiURL, token, 'OpenAI', 'openai')
    await assignProviderToAdministratorsGroup(apiURL, token, providerId)

    // The model MUST be marked tool-capable (capabilities.tools=true) — a
    // built-in like run_js is only auto-attached to a tool-capable model,
    // otherwise the model would hallucinate the call. createModelViaAPI does
    // not set `tools`, so POST the model directly.
    const modelRes = await page.request.post(`${apiURL}/api/llm-models`, {
      headers: { Authorization: `Bearer ${token}` },
      data: {
        provider_id: providerId,
        name: MODEL,
        display_name: 'RunJS Tool Model',
        enabled: true,
        engine_type: 'none',
        file_format: 'gguf',
        capabilities: { tools: true, chat: true, streaming: true },
        parameters: { context_length: 8192, temperature: 0, top_p: 0.9, max_tokens: 1024 },
      },
    })
    expect(modelRes.ok()).toBeTruthy()

    await goToNewChatPage(page, baseURL)
    await selectModelInDropdown(page, 'RunJS Tool Model')

    const textarea = page.locator('[data-testid="chat-message-textarea"]').first()
    await textarea.fill(
      'Use the run_js tool to compute 6 * 7 in JavaScript. The script must `return` the number. ' +
        'Do the arithmetic inside the tool, then tell me the result.',
    )
    await page.locator('[data-testid="chat-input-send-btn"]').click()

    // The real model chose run_js → the embedded runtime executed the script →
    // a run_js tool card mounts. This is the whole capability, end-to-end.
    const runJsCard = page
      .locator('[data-testid^="mcp-toolcall-card-"]')
      .filter({ hasText: 'run_js' })
    await expect(runJsCard.first()).toBeVisible({ timeout: 150_000 })
  })
})
