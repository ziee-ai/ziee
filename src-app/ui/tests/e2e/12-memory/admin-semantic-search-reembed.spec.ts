import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'

/**
 * E2E — Memory admin SemanticSearchSection: change the embedding model + save
 * (background re-embed message) and the explicit "Re-embed now" confirm flow.
 *
 * Untested high-value path: picking an embedding model and saving must surface
 * "Embedding model changed — re-embed running in background.", and the
 * "Re-embed now" button must open the confirm modal and dispatch the job.
 */

async function createProvider(
  request: import('@playwright/test').APIRequestContext,
  apiURL: string,
  token: string,
): Promise<string> {
  const res = await request.post(`${apiURL}/api/llm-providers`, {
    headers: { Authorization: `Bearer ${token}` },
    data: {
      name: `sem-prov-${Date.now().toString(36)}`,
      provider_type: 'openai',
      enabled: false,
      api_key: 'sk-test123',
    },
  })
  expect(res.status()).toBe(201)
  return (await res.json()).id
}

async function createEmbeddingModel(
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
      description: 'e2e embedding model',
      enabled: true,
      engine_type: 'none',
      file_format: 'gguf',
      capabilities: { text_embedding: true },
    },
  })
  expect(res.status()).toBe(201)
}

test.describe('Memory — admin semantic search re-embed', () => {
  test('selecting an embedding model + saving triggers the background re-embed message', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)

    const embedName = `e2e-embed-sem-${Date.now().toString(36)}`
    const providerId = await createProvider(page.request, apiURL, adminToken)
    await createEmbeddingModel(page.request, apiURL, adminToken, providerId, embedName)

    await page.goto(`${baseURL}/settings/memory-admin`)
    const card = page.locator(
      '.ant-card:has(.ant-card-head-title:has-text("Semantic search"))',
    )
    await expect(card).toBeVisible({ timeout: 30000 })

    // Enable semantic search.
    const enableSwitch = card.getByRole('switch', {
      name: 'Enable semantic search retrieval',
    })
    if ((await enableSwitch.getAttribute('aria-checked')) === 'false') {
      await enableSwitch.click()
    }

    // Pick the seeded embedding model.
    await card
      .locator('.ant-form-item:has-text("Embedding model")')
      .first()
      .getByRole('combobox')
      .click()
    await page
      .locator('.ant-select-dropdown:not(.ant-select-dropdown-hidden)')
      .locator('.ant-select-item-option')
      .filter({ hasText: embedName })
      .click()

    // Save → since the embedding model changed from none, the background
    // re-embed message is surfaced.
    await card.getByRole('button', { name: 'Save', exact: true }).click()
    await expect(
      page.getByText(/Embedding model changed — re-embed running in background\./),
    ).toBeVisible({ timeout: 10000 })

    // Now the explicit "Re-embed now" affordance: opens the confirm modal and
    // dispatches the job.
    await card.getByRole('button', { name: 'Re-embed now' }).click()
    const modal = page.getByRole('dialog', { name: 'Re-embed every memory?' })
    await expect(modal).toBeVisible({ timeout: 10000 })
    await modal.getByRole('button', { name: 'Re-embed', exact: true }).click()
    await expect(
      page.getByText(/Re-embed job dispatched in background/),
    ).toBeVisible({ timeout: 10000 })
  })
})
