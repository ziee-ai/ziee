import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  createModelViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'
import { byTestId } from '../testid'
import {
  fillProjectForm,
  getProjectCard,
  goToProjectsPage,
  openCreateProjectDrawer,
  submitProjectForm,
} from './helpers/project-helpers'

/**
 * ProjectDefaultsForm interaction (detail-page-layout.spec only asserts the
 * "false" status). This picks a default model from the Advanced section's
 * Select and asserts the PUT persists (toast + the data-test flag flips to
 * 'true'). No LLM call — setting a default only writes the FK.
 */
test.describe('Projects - default assistant/model selection', () => {
  test('selecting a default model persists it', async ({ page, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)

    // A provider + model must exist for the picker to offer an option.
    const adminToken = await getAdminToken(apiURL)
    const providerId = await createProviderViaAPI(
      apiURL,
      adminToken,
      'OpenAI',
      'openai',
    )
    await assignProviderToAdministratorsGroup(apiURL, adminToken, providerId)
    const modelId = await createModelViaAPI(
      apiURL,
      adminToken,
      providerId,
      'gpt-4o-mini',
      'GPT-4o Mini',
      'openai',
    )

    await goToProjectsPage(page, baseURL)
    await openCreateProjectDrawer(page)
    await fillProjectForm(page, { name: 'Defaults Probe' })
    await submitProjectForm(page)

    await getProjectCard(page, 'Defaults Probe').click()
    await page.waitForURL(/\/projects\/[0-9a-f-]+$/)

    const advanced = page.locator('[data-test-section="advanced"]')
    await expect(advanced).toBeVisible({ timeout: 15000 })
    // Initially no default model.
    await expect(
      advanced.locator('[data-test-default-model-set="false"]'),
    ).toBeVisible()

    // Open the "Default model" combobox and choose the seeded model
    // (kit Combobox derives option testids as `<testid>-opt-<value>`).
    await byTestId(advanced, 'project-default-model-combobox').click()
    await byTestId(page, `project-default-model-combobox-opt-${modelId}`).click()

    // Persisted: success toast + the status flag flips to 'true'.
    await expect(
      page.locator('[data-sonner-toast][data-type="success"]').first(),
    ).toBeVisible()
    await expect(
      advanced.locator('[data-test-default-model-set="true"]'),
    ).toBeVisible()
  })
})
