import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import { byTestId } from '../testid'

/**
 * E2E — the Memory admin page must offer the right models in each picker.
 *
 * Regression for the bug where the "Default extraction model" dropdown
 * reused the embedding-only model list, so an admin could only ever pick
 * the embedding model there — which then 500s the extraction pipeline
 * ("the current context does not support logits computation").
 *
 * Contract:
 *   - Embedding model picker  → embedding-capable models only.
 *   - Default extraction model picker → non-embedding (chat/generation)
 *     models only.
 */

async function createProvider(
  request: import('@playwright/test').APIRequestContext,
  apiURL: string,
  token: string,
): Promise<string> {
  const res = await request.post(`${apiURL}/api/llm-providers`, {
    headers: { Authorization: `Bearer ${token}` },
    data: {
      name: `ext-dropdown-prov-${Date.now().toString(36)}`,
      provider_type: 'openai',
      enabled: false,
      api_key: 'sk-test123',
    },
  })
  expect(res.status()).toBe(201)
  return (await res.json()).id
}

async function createModel(
  request: import('@playwright/test').APIRequestContext,
  apiURL: string,
  token: string,
  providerId: string,
  name: string,
  capabilities: Record<string, boolean>,
): Promise<void> {
  const res = await request.post(`${apiURL}/api/llm-models`, {
    headers: { Authorization: `Bearer ${token}` },
    data: {
      provider_id: providerId,
      name,
      display_name: name,
      description: 'e2e extraction-dropdown test model',
      enabled: true,
      engine_type: 'none',
      file_format: 'gguf',
      capabilities,
    },
  })
  expect(res.status()).toBe(201)
}

test.describe('Memory — admin extraction-model picker', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    await loginAsAdmin(page, testInfra.baseURL)
  })

  test('extraction picker shows chat models, embedding picker shows embedders', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    const adminToken = await getAdminToken(apiURL)

    const suffix = Date.now().toString(36)
    const embedName = `e2e-embed-${suffix}`
    const chatName = `e2e-chat-${suffix}`

    // Seed one embedding model and one chat model BEFORE loading the page
    // so the admin form's model fetches pick both up.
    const providerId = await createProvider(page.request, apiURL, adminToken)
    await createModel(page.request, apiURL, adminToken, providerId, embedName, {
      text_embedding: true,
    })
    await createModel(page.request, apiURL, adminToken, providerId, chatName, {
      chat: true,
    })

    await page.goto(`${baseURL}/settings/memory-admin`)
    await expect(byTestId(page, 'memory-extraction-model-combobox')).toBeVisible({
      timeout: 30000,
    })

    // Assert on the option ELEMENT (role=option), filtered by the model name
    // the test itself created (dynamic data). Count assertions auto-retry
    // while the async model fetch settles.
    const option = (name: string) =>
      page.getByRole('option').filter({ hasText: name })

    // ── Extraction picker → chat model present, embedding model absent ──
    await byTestId(page, 'memory-extraction-model-combobox').click()
    await expect(option(chatName)).toHaveCount(1)
    await expect(option(embedName)).toHaveCount(0)

    // Close before opening the next picker; assert it actually closed so the
    // option locator can't match two open popovers at once.
    await page.keyboard.press('Escape')
    await expect(page.getByRole('option')).toHaveCount(0)

    // ── Embedding picker → embedding model present, chat model absent ──
    await byTestId(page, 'memory-semantic-model-combobox').click()
    await expect(option(embedName)).toHaveCount(1)
    await expect(option(chatName)).toHaveCount(0)
  })
})
