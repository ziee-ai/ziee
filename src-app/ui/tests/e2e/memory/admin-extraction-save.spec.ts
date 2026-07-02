import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import { byTestId } from '../testid'

/**
 * E2E — saving the Memory admin "Default extraction model" persists.
 *
 * Audit gap: ExtractionSection.tsx's `handleSubmit` (select model →
 * Stores.MemoryAdmin.update → "Extraction settings saved." toast) was never
 * exercised. The sibling spec `admin-extraction-model-dropdown.spec.ts` only
 * asserts which models populate the picker — it never SAVES. This drives the
 * full pick→save→persist path that configures which LLM the silent
 * auto-extraction pipeline defaults to (the chat-side after_llm_call hook
 * reads this value).
 *
 * Real backend throughout — only a chat-capable model is seeded via the
 * public REST API so the picker has something to select.
 */
async function seedChatModel(
  request: import('@playwright/test').APIRequestContext,
  apiURL: string,
  token: string,
  name: string,
): Promise<void> {
  const provRes = await request.post(`${apiURL}/api/llm-providers`, {
    headers: { Authorization: `Bearer ${token}` },
    data: {
      name: `mem-extract-save-prov-${Date.now().toString(36)}`,
      provider_type: 'openai',
      enabled: false,
      api_key: 'sk-test123',
    },
  })
  expect(provRes.status()).toBe(201)
  const providerId = (await provRes.json()).id

  const modelRes = await request.post(`${apiURL}/api/llm-models`, {
    headers: { Authorization: `Bearer ${token}` },
    data: {
      provider_id: providerId,
      name,
      display_name: name,
      description: 'e2e extraction-save test model',
      enabled: true,
      engine_type: 'none',
      file_format: 'gguf',
      capabilities: { chat: true },
    },
  })
  expect(modelRes.status()).toBe(201)
}

test.describe('Memory — admin extraction model save', () => {
  test('select an extraction model → Save → success toast', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)
    const chatName = `e2e-extract-save-${Date.now().toString(36)}`
    await seedChatModel(page.request, apiURL, adminToken, chatName)

    await page.goto(`${baseURL}/settings/memory-admin`)
    await expect(byTestId(page, 'memory-extraction-model-combobox')).toBeVisible({
      timeout: 30000,
    })

    // Open the extraction picker and choose the seeded chat model.
    await byTestId(page, 'memory-extraction-model-combobox').click()
    await page.getByRole('option').filter({ hasText: chatName }).first().click()

    // Save the extraction card.
    await byTestId(page, 'memory-extraction-save-btn').click()

    await expect(page.locator('[data-sonner-toast]')).toContainText(
      'Extraction settings saved.',
      { timeout: 10000 },
    )

    // Reload → the saved selection is reflected (persisted, not transient). The
    // kit Combobox trigger is an <input role="combobox"> whose selected label
    // lives in its `value` attribute, so assert with toHaveValue (not
    // toContainText, which reads text content an <input> never has).
    await page.reload()
    await expect(
      byTestId(page, 'memory-extraction-model-combobox'),
    ).toHaveValue(chatName, { timeout: 30000 })
  })
})
