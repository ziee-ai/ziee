import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  createModelViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'
import {
  goToWorkflowsSettingsPage,
  openWorkflowCard,
  seedDevWorkflow,
} from './helpers/workflow-helpers'

/**
 * E2E — WorkflowRunProgressView MID-RUN CANCEL (WorkflowRunProgressView.tsx
 * Cancel button → Stores.WorkflowRun.cancel). A multi-step llm workflow gives a
 * window to click Cancel before completion; the run must reach status
 * "cancelled". Real-LLM gated.
 */

const ANTHROPIC_KEY = process.env.ANTHROPIC_API_KEY ?? ''

// Several llm steps so the run stays non-terminal long enough to cancel.
const MULTI_STEP_YAML = `$schema: "/schemas/2026-06-12/workflow-definition.schema.json"
inputs:
  - name: topic
    required: true
steps:
  - id: s1
    kind: llm
    message: "Step 1 on {{ inputs.topic }}"
    prompt: |
      Write five sentences about "{{ inputs.topic }}".
  - id: s2
    kind: llm
    message: "Step 2"
    prompt: |
      Write five more sentences building on the previous about "{{ inputs.topic }}".
  - id: s3
    kind: llm
    message: "Step 3"
    prompt: |
      Write a final five sentences about "{{ inputs.topic }}".
outputs:
  - name: out
    from: "{{ s3.output }}"
    expose: full
`

test.describe('Workflows - mid-run cancel', () => {
  test.skip(ANTHROPIC_KEY.length === 0, 'ANTHROPIC_API_KEY not set — real-LLM cancel skipped')

  test('clicking Cancel during a run drives it to cancelled', async ({
    page,
    request,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    const providerId = await createProviderViaAPI(apiURL, token, 'Anthropic', 'anthropic')
    await assignProviderToAdministratorsGroup(apiURL, token, providerId)
    await createModelViaAPI(
      apiURL,
      token,
      providerId,
      'claude-haiku-4-5-20251001',
      'Claude Haiku 4.5',
      'anthropic',
    )
    await seedDevWorkflow(request, apiURL, token, 'e2e-cancel-run', MULTI_STEP_YAML)

    await goToWorkflowsSettingsPage(page, baseURL)
    await openWorkflowCard(page, 'e2e-cancel-run')

    await page.getByRole('button', { name: /Run$/ }).first().click()
    await expect(page.getByRole('dialog', { name: /^Run / })).toBeVisible({ timeout: 10000 })
    const topic = page.getByLabel('topic')
    if (await topic.count()) await topic.first().fill('the history of computing')
    else await page.getByPlaceholder(/"topic"/).fill('{ "topic": "the history of computing" }')
    await page.getByLabel('Model').click()
    await page.getByRole('option', { name: /Claude Haiku 4\.5/ }).first().click()
    await page.getByRole('button', { name: 'Run', exact: true }).last().click()

    // The progress view appears; cancel while the run is still in flight.
    await expect(page.getByText('Run progress')).toBeVisible({ timeout: 15000 })
    await page.getByRole('button', { name: 'Cancel' }).click()

    // The run reaches the cancelled terminal state.
    await expect(
      page.getByText('cancelled', { exact: true }).first(),
    ).toBeVisible({ timeout: 60000 })
  })
})
