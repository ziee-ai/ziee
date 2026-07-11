import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  createModelViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'
import {
  goToProvidersPage,
  waitForProvidersPageLoad,
  clickProviderCard,
} from './helpers/navigation-helpers'
import { openEditModelDrawer } from './helpers/model-helpers'
import { byTestId } from '../testid'

/**
 * TEST-21 (ITEM-14): the per-model parameter-contract override in the Edit
 * drawer (LlmModelCapabilitiesSection). Toggle "Supports sampling params" to
 * "No", save, and confirm the override persists on the model row (the editable
 * source of truth the provider adapter reads).
 */
test.describe('LLM Models - parameter-contract capability toggle', () => {
  test('sets supports_sampling_params to No and it persists', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)

    const providerName = `caps-toggle-${Date.now()}`
    const providerId = await createProviderViaAPI(
      apiURL,
      adminToken,
      providerName,
      'openai',
    )
    await assignProviderToAdministratorsGroup(apiURL, adminToken, providerId)
    const modelId = await createModelViaAPI(
      apiURL,
      adminToken,
      providerId,
      'gpt-4o-mini',
      'Toggle Model',
      'openai',
    )

    await goToProvidersPage(page, baseURL)
    await waitForProvidersPageLoad(page)
    await clickProviderCard(page, providerName)
    await expect(page).toHaveURL(/\/settings\/llm-providers\/[a-f0-9-]+/)

    // Open the edit drawer and set the sampling-params contract to "No".
    await openEditModelDrawer(page, 'Toggle Model')
    await byTestId(page, 'llm-capability-select-supports_sampling_params').click()
    await byTestId(
      page,
      'llm-capability-select-supports_sampling_params-opt-false',
    ).click()

    const [saveResp] = await Promise.all([
      page.waitForResponse(
        r =>
          /\/api\/llm-models\/[0-9a-f-]+$/.test(r.url()) &&
          r.request().method() === 'POST',
        { timeout: 15000 },
      ),
      byTestId(page, 'llm-edit-model-save-btn').click(),
    ])
    expect(saveResp.ok()).toBeTruthy()

    // The override persists on the row (read back via the API).
    const got = await fetch(`${apiURL}/api/llm-models/${modelId}`, {
      headers: { Authorization: `Bearer ${adminToken}` },
    })
    expect(got.ok).toBeTruthy()
    const body = await got.json()
    expect(body.capabilities?.supports_sampling_params).toBe(false)
  })
})
