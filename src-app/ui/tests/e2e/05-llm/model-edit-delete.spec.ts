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
import {
  openEditModelDrawer,
  deleteModel,
  assertModelExists,
  assertModelNotExists,
} from './helpers/model-helpers'
import { byTestId } from '../testid'

/**
 * Model edit + delete from the provider detail page (LlmModelsSection +
 * EditLlmModelDrawer). The model list's per-row Edit/Delete actions were
 * untested. Seed a provider+model via API, then drive the UI.
 */
test.describe('LLM Models - edit + delete from provider detail', () => {
  test('edits a model display name then deletes it', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)

    const providerName = `model-crud-${Date.now()}`
    const providerId = await createProviderViaAPI(
      apiURL,
      adminToken,
      providerName,
      'openai',
    )
    await assignProviderToAdministratorsGroup(apiURL, adminToken, providerId)
    await createModelViaAPI(
      apiURL,
      adminToken,
      providerId,
      'gpt-4o-mini',
      'Original Model Name',
      'openai',
    )

    await goToProvidersPage(page, baseURL)
    await waitForProvidersPageLoad(page)
    await clickProviderCard(page, providerName)
    await expect(page).toHaveURL(/\/settings\/llm-providers\/[a-f0-9-]+/)

    await assertModelExists(page, 'Original Model Name')

    // EDIT: change the display name and save.
    await openEditModelDrawer(page, 'Original Model Name')
    await byTestId(page, 'llm-param-display_name').fill('Renamed Model')
    const [editResp] = await Promise.all([
      page.waitForResponse(
        r => /\/api\/.*models/.test(r.url()) && r.request().method() === 'PUT',
        { timeout: 15000 },
      ),
      byTestId(page, 'llm-edit-model-save-btn').click(),
    ])
    expect(editResp.ok()).toBeTruthy()
    await assertModelExists(page, 'Renamed Model')

    // DELETE: remove the model from the list.
    await deleteModel(page, 'Renamed Model')
    await assertModelNotExists(page, 'Renamed Model')
  })
})
