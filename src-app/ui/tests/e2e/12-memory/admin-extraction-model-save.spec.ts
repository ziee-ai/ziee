import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'

/**
 * E2E — Memory admin ExtractionSection: changing the default extraction model
 * and SAVING it (the sibling dropdown spec only asserts which options appear).
 *
 * Picks a chat model in the "Default extraction model" select, clicks Save,
 * asserts the "Extraction settings saved." toast, then reloads and asserts the
 * selection persisted (the PUT actually took effect).
 */

async function createProvider(
  request: import('@playwright/test').APIRequestContext,
  apiURL: string,
  token: string,
): Promise<string> {
  const res = await request.post(`${apiURL}/api/llm-providers`, {
    headers: { Authorization: `Bearer ${token}` },
    data: {
      name: `ext-save-prov-${Date.now().toString(36)}`,
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
): Promise<void> {
  const res = await request.post(`${apiURL}/api/llm-models`, {
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
  expect(res.status()).toBe(201)
}

test.describe('Memory — admin extraction-model save', () => {
  test('selecting an extraction model and saving persists it', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)

    const chatName = `e2e-extract-save-${Date.now().toString(36)}`
    const providerId = await createProvider(page.request, apiURL, adminToken)
    await createModel(page.request, apiURL, adminToken, providerId, chatName)

    await page.goto(`${baseURL}/settings/memory-admin`)
    await expect(page.getByText(/Default extraction model/)).toBeVisible({
      timeout: 30000,
    })

    // Open the extraction picker and choose the seeded chat model.
    await page
      .locator('.ant-form-item:has-text("Default extraction model")')
      .first()
      .getByRole('combobox')
      .click()
    const dropdown = page.locator(
      '.ant-select-dropdown:not(.ant-select-dropdown-hidden)',
    )
    await expect(dropdown).toBeVisible()
    await dropdown
      .locator('.ant-select-item-option')
      .filter({ hasText: chatName })
      .click()

    // Save → success toast.
    await page.getByRole('button', { name: 'Save', exact: true }).click()
    await expect(page.getByText('Extraction settings saved.')).toBeVisible({
      timeout: 10000,
    })

    // Reload → the chosen model is still selected (PUT persisted).
    await page.reload()
    await expect(
      page
        .locator('.ant-form-item:has-text("Default extraction model")')
        .getByTitle(chatName)
        .or(
          page
            .locator('.ant-form-item:has-text("Default extraction model")')
            .locator('.ant-select-selection-item', { hasText: chatName }),
        ),
    ).toBeVisible({ timeout: 30000 })
  })
})
