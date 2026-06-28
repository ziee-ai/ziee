import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import {
  goToWorkflowsSettingsPage,
  openWorkflowCard,
  seedDevWorkflow,
} from './helpers/workflow-helpers'

/**
 * Workflow run-dialog error handling (WorkflowRunDialog.handleRun). The dialog
 * guards against starting a run with no model selected — a deterministic error
 * path that needs no provider/model/LLM (fresh deploy → empty model picker).
 */
const WF_YAML = `$schema: "/schemas/2026-06-12/workflow-definition.schema.json"
inputs:
  - name: topic
    required: false
steps:
  - id: noop
    kind: llm
    prompt: "about {{ inputs.topic }}"
outputs:
  - name: out
    from: "{{ noop.output }}"
    expose: full
`

test.describe('Workflows - run dialog error handling', () => {
  test('running with no model selected shows a validation error', async ({
    page,
    request,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)

    const slug = `e2e-run-err-${Date.now()}`
    await seedDevWorkflow(request, apiURL, adminToken, slug, WF_YAML)

    await goToWorkflowsSettingsPage(page, baseURL)
    await openWorkflowCard(page, slug)

    // Open the Run dialog from the drawer.
    await page.getByRole('button', { name: /Run$/ }).first().click()
    const dialog = page.getByRole('dialog', { name: /^Run / })
    await expect(dialog).toBeVisible({ timeout: 10000 })

    // No model is configured on a fresh deploy → the picker is empty. Clicking
    // Run hits the "Select a model to run this workflow" guard (handleRun).
    await dialog.getByRole('button', { name: 'Run', exact: true }).click()
    await expect(
      page.getByText('Select a model to run this workflow'),
    ).toBeVisible({ timeout: 10000 })

    // The dialog stays open (the run never started).
    await expect(dialog).toBeVisible()
  })
})
