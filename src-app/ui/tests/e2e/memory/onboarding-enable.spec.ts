import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'
import {
  loginAsAdmin,
  getAdminToken,
  getCurrentUserToken,
  createTestUser,
  loginExpectingOnboarding,
} from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  assignProviderToAdministratorsGroup,
  getAdministratorsGroupId,
  assignUserToGroupViaAPI,
} from '../../common/provider-helpers'

/**
 * E2E — onboarding Memory step: ENABLE path.
 *
 * Plan §9 Phase 1: "admin walks through onboarding, picks Enable +
 * Download nomic-embed-text from HuggingFace + selects it; assert
 * settings update and the Memories page is reachable."
 *
 * The Memory step's model dropdown is gated on `?capability=text_embedding`,
 * so we seed an embedding-capable OpenAI model via API in `beforeEach`
 * (the same admin token onboarding uses). This sidesteps the real
 * HuggingFace download path (which the dropdown also supports) while
 * still exercising the full capability-filter → pick → settings-update
 * surface end-to-end.
 */

/**
 * Seed an embedding-capable OpenAI model. The OpenAI provider gets
 * created with the live `OPENAI_API_KEY`, the model is registered
 * with `capabilities.text_embedding = true` so the Memory admin
 * dropdown surfaces it, and it's assigned to the Administrators group
 * so the admin onboarder can see it.
 */
async function seedEmbeddingModel(apiURL: string, adminToken: string): Promise<string> {
  const providerId = await createProviderViaAPI(apiURL, adminToken, 'OpenAI Embeddings', 'openai')
  await assignProviderToAdministratorsGroup(apiURL, adminToken, providerId)

  const res = await fetch(`${apiURL}/api/llm-models`, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      Authorization: `Bearer ${adminToken}`,
    },
    body: JSON.stringify({
      provider_id: providerId,
      name: 'text-embedding-3-small',
      display_name: 'Text Embedding 3 Small',
      enabled: true,
      engine_type: 'none',
      file_format: 'gguf',
      capabilities: {
        vision: false,
        function_calling: false,
        streaming: false,
        text_embedding: true,
      },
      parameters: {
        context_length: 8191,
      },
    }),
  })
  if (!res.ok) {
    const body = await res.text()
    throw new Error(`Failed to seed embedding model: ${res.status} ${body}`)
  }
  const model = await res.json()
  return model.id
}

test.describe('Memory — onboarding enable', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    await loginAsAdmin(page, testInfra.baseURL)
    // Seed the embedding model BEFORE creating the onboarding user so
    // the Memory step's capability-filtered dropdown has an option.
    const adminToken = await getAdminToken(testInfra.apiURL)
    await seedEmbeddingModel(testInfra.apiURL, adminToken)
  })

  test('admin enables memory + picks model', async ({ page, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    const adminToken = await getAdminToken(apiURL)
    const username = `enable_${Date.now().toString(36)}`
    const userId = await createTestUser(
      apiURL,
      adminToken,
      username,
      `${username}@ex.com`,
      'password123',
      [
        'profile::read',
        'profile::edit',
        'memory::read',
        'memory::write',
        'memory::admin::read',
        'memory::admin::manage',
        // `llm_models::read` is what the Memory step's
        // capability-filtered model dropdown queries against; without
        // it `/api/llm-models?capability=text_embedding` 403s and the
        // dropdown stays empty.
        'llm_models::read',
      ],
    )

    // Membership in Administrators is required to SEE the seeded
    // provider's models (the seeded provider is assigned to that
    // group; users outside it get a filtered-empty list even with
    // llm_models::read).
    const adminGroupId = await getAdministratorsGroupId(apiURL, adminToken)
    await assignUserToGroupViaAPI(apiURL, adminToken, userId, adminGroupId)

    await loginExpectingOnboarding(page, baseURL, username, 'password123')

    await byTestId(page, 'onboarding-page-next-button').click() // Welcome
    await byTestId(page, 'onboarding-page-next-button').click() // API Keys
    await byTestId(page, 'onboarding-page-next-button').click() // MCP

    // Memory step: the enable switch only renders here, so it confirms the
    // step + is the toggle we flip on.
    await expect(
      byTestId(page, 'onboarding-memory-enable-switch'),
    ).toBeVisible()
    await byTestId(page, 'onboarding-memory-enable-switch').click()
    // Pick the first option in the embedding-model dropdown. Opening the kit
    // Select and using keyboard selection is robust regardless of option
    // values (which we don't know up front).
    await byTestId(page, 'onboarding-memory-model-select').click()
    await page.keyboard.press('ArrowDown')
    await page.keyboard.press('Enter')

    await byTestId(page, 'onboarding-page-next-button').click()
    // Final step — the last-step button shares the next-button testid
    // (label flips to "Start Chatting").
    await byTestId(page, 'onboarding-page-next-button').click()

    // Verify settings now enabled.
    const userToken = await getCurrentUserToken(page)
    const adminRes = await page.request.get(
      `${apiURL}/api/memory/admin-settings`,
      { headers: { Authorization: `Bearer ${userToken}` } },
    )
    const settings = await adminRes.json()
    expect(settings.enabled).toBe(true)
    expect(settings.embedding_model_id).not.toBeNull()
  })
})
