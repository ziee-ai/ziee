import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import {
  goToWorkflowsSettingsPage,
  openWorkflowCard,
  seedDevWorkflow,
} from './helpers/workflow-helpers'
import { byTestId } from '../testid'

/**
 * E2E — WorkflowRunDialog input validation.
 *
 * `WorkflowRunDialog` enforces required structured inputs via antd Form rules
 * ("{name} is required") and blocks the run when no model is selected
 * ("Select a model to run this workflow"). No LLM is needed — validation fires
 * before any run is submitted.
 */

const REQUIRED_INPUT_YAML = `$schema: "/schemas/2026-06-12/workflow-definition.schema.json"
inputs:
  - name: topic
    description: "subject to summarize"
    required: true
steps:
  - id: summarize
    kind: llm
    message: "Summarizing {{ inputs.topic }}"
    prompt: |
      One sentence about "{{ inputs.topic }}".
outputs:
  - name: summary
    from: "{{ summarize.output }}"
    expose: full
`

test.describe('Workflows - run dialog input validation', () => {
  test('clicking Run with an empty required input shows an inline error', async ({
    page,
    request,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra

    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)

    await seedDevWorkflow(
      request,
      apiURL,
      adminToken,
      'e2e-run-validation',
      REQUIRED_INPUT_YAML,
    )

    await goToWorkflowsSettingsPage(page, baseURL)
    await openWorkflowCard(page, 'e2e-run-validation')

    // Open the Run dialog.
    await byTestId(page, 'wf-detail-run-btn').click()
    const dialog = byTestId(page, 'wf-run-dialog')
    await expect(dialog).toBeVisible({ timeout: 10000 })

    // Click Run without filling the required `topic` input.
    await byTestId(page, 'wf-run-submit-btn').click()

    // The Form rule surfaces an inline "topic is required" error and the
    // dialog stays open (no run submitted).
    await expect(byTestId(page, 'wf-run-form')).toContainText('topic is required', {
      timeout: 10000,
    })
    await expect(dialog).toBeVisible()
  })
})
