import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  createModelViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'
import { byTestId } from '../testid'

/**
 * E2E — ProjectDefaultsForm (audit id all-f5646555310e).
 *
 * The project detail page's Advanced card hosts an inline, auto-save
 * form with two antd Selects: "Default assistant" and "Default model".
 * Each onChange fires `PUT /api/projects/{id}` (via
 * Stores.Projects.updateProject) writing default_assistant_id /
 * default_model_id, which then snapshots onto every new conversation
 * created in the project.
 *
 * Existing projects/detail-page-layout.spec.ts only asserts the
 * EMPTY ("false") summary state. This drives the real selection flow:
 * pick an assistant + model, assert each PUT fires with the right
 * field, and assert the choices PERSIST across a reload (the wrapper
 * `data-test-default-*-set` flags flip to "true").
 *
 * Nothing is mocked — real provider/model/assistant seeding via API,
 * real handler, real DB write.
 */

test.describe('Projects — default assistant/model selection', () => {
  test('selecting a default assistant and model persists via PUT', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)

    const tag = Date.now().toString(36)

    // ---- Seed an enabled, admin-accessible model (gives the model
    // ---- picker an option). createModelViaAPI defaults to display
    // ---- name "GPT-4o Mini".
    const providerId = await createProviderViaAPI(
      apiURL,
      adminToken,
      `Defaults Provider ${tag}`,
      'openai',
    )
    await assignProviderToAdministratorsGroup(apiURL, adminToken, providerId)
    const modelId = await createModelViaAPI(
      apiURL,
      adminToken,
      providerId,
      undefined,
      `Defaults Model ${tag}`,
      'openai',
    )

    // ---- Seed an assistant (gives the assistant picker an option).
    const assistantName = `Defaults Assistant ${tag}`
    const aRes = await fetch(`${apiURL}/api/assistants`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        Authorization: `Bearer ${adminToken}`,
      },
      body: JSON.stringify({
        name: assistantName,
        instructions: 'You are a defaults-test assistant.',
      }),
    })
    expect(aRes.ok, `create assistant: ${aRes.status}`).toBeTruthy()
    const assistant = await aRes.json()

    // ---- Seed a project via API.
    const projRes = await fetch(`${apiURL}/api/projects`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        Authorization: `Bearer ${adminToken}`,
      },
      body: JSON.stringify({ name: `Defaults Project ${tag}` }),
    })
    expect(projRes.ok, `create project: ${projRes.status}`).toBeTruthy()
    const project = await projRes.json()
    const projectId: string = project.id

    await page.goto(`${baseURL}/projects/${projectId}`)
    await page
      .locator('[data-test-project-title]')
      .first()
      .waitFor({ state: 'visible', timeout: 15000 })

    const advanced = page.locator('[data-test-section="advanced"]')
    await expect(advanced).toBeVisible()

    // Both defaults start unset.
    await expect(
      advanced.locator('[data-test-default-assistant-set="false"]'),
    ).toBeVisible()
    await expect(
      advanced.locator('[data-test-default-model-set="false"]'),
    ).toBeVisible()

    const putMatches = (urlEnd: string) => (req: import('@playwright/test').Request) =>
      req.method() === 'PUT' &&
      new RegExp(`/api/projects/${projectId}$`).test(req.url()) &&
      (req.postData() ?? '').includes(urlEnd)

    // -------------------- Default assistant --------------------
    await byTestId(advanced, 'project-default-assistant-combobox').click()

    const assistantPut = page.waitForRequest(
      putMatches('default_assistant_id'),
    )
    await byTestId(
      page,
      `project-default-assistant-combobox-opt-${assistant.id}`,
    ).click()
    const assistantReq = await assistantPut
    expect(assistantReq.postData()).toContain(assistant.id)

    // -------------------- Default model --------------------
    await byTestId(advanced, 'project-default-model-combobox').click()

    const modelPut = page.waitForRequest(putMatches('default_model_id'))
    await byTestId(
      page,
      `project-default-model-combobox-opt-${modelId}`,
    ).click()
    const modelReq = await modelPut
    expect(modelReq.postData()).toContain(modelId)

    // -------------------- Persistence across reload --------------------
    await page.reload()
    await page
      .locator('[data-test-project-title]')
      .first()
      .waitFor({ state: 'visible', timeout: 15000 })

    const advancedAfter = page.locator('[data-test-section="advanced"]')
    await expect(
      advancedAfter.locator('[data-test-default-assistant-set="true"]'),
    ).toBeVisible({ timeout: 10000 })
    await expect(
      advancedAfter.locator('[data-test-default-model-set="true"]'),
    ).toBeVisible()
  })
})
