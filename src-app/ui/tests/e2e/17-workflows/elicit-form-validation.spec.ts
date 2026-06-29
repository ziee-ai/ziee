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
import { byTestId } from '../testid'

/**
 * E2E — WorkflowElicitForm submit-VALIDATION error display
 * (WorkflowElicitForm.tsx:278-311 handleSubmit catch → setError, :350-354 error
 * Alert). The rich-table spec only drives the happy path; submitting with a
 * REQUIRED field empty (→ the top-level "Please fix the highlighted fields"
 * Alert, run NOT resumed) was untested. Real-LLM snapshot tier.
 */

const ANTHROPIC_KEY = process.env.ANTHROPIC_API_KEY ?? ''

// `gen` (llm, mocked) → `ask` (elicit) with a single REQUIRED string field.
const REQUIRED_FIELD_YAML = `$schema: "/schemas/2026-06-12/workflow-definition.schema.json"
inputs:
  - name: topic
    required: false
steps:
  - id: gen
    kind: llm
    prompt: "gen {{ inputs.topic }}"
    output_format: json
  - id: ask
    kind: elicit
    message: "Provide a required label"
    data: "{{ gen.output }}"
    schema:
      type: object
      properties:
        label:
          type: string
      required: [label]
    timeout_ms: 600000
    depends_on: [gen]
outputs:
  - name: out
    from: "{{ ask.output }}"
    expose: full
`

test.describe('Workflows - elicit form submit validation', () => {
  test.skip(ANTHROPIC_KEY.length === 0, 'ANTHROPIC_API_KEY not set — real-LLM elicit validation skipped')

  test('submitting with a required field empty shows the validation error', async ({
    page,
    request,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)
    const providerId = await createProviderViaAPI(apiURL, adminToken, 'Anthropic', 'anthropic')
    await assignProviderToAdministratorsGroup(apiURL, adminToken, providerId)
    const modelId = await createModelViaAPI(
      apiURL,
      adminToken,
      providerId,
      'claude-haiku-4-5-20251001',
      'Claude Haiku 4.5',
      'anthropic',
    )
    const workflowId = await seedDevWorkflow(
      request,
      apiURL,
      adminToken,
      'e2e-elicit-validate',
      REQUIRED_FIELD_YAML,
    )

    // Start the run with `gen` mocked to an empty object (no label seeded), so
    // the elicit form opens with the required `label` field UNFILLED.
    const runResp = await request.post(`${apiURL}/api/workflows/${workflowId}/run`, {
      headers: { Authorization: `Bearer ${adminToken}` },
      data: { inputs: { topic: 'x' }, model_id: modelId, mocks: { gen: {} } },
    })
    expect(runResp.status(), `run should 202: ${await runResp.text()}`).toBe(202)

    await goToWorkflowsSettingsPage(page, baseURL)
    await openWorkflowCard(page, 'e2e-elicit-validate')
    await page.locator('[data-testid^="wf-run-source-tag-"]').first().click()
    await expect(byTestId(page, 'wf-elicit-alert')).toBeVisible({ timeout: 15000 })

    // Submit without filling the required `label` → the validation Alert renders.
    await byTestId(page, 'wf-elicit-submit-btn').click()
    await expect(byTestId(page, 'wf-elicit-error-alert')).toBeVisible({
      timeout: 10000,
    })
  })
})
