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

// audit ids all-ea879b49efc5 (dynamic form rendering from JSON schema — all
// field types) + all-fa60163cc857 (validation error display on submit).
// Drives WorkflowElicitForm via a real (model-snapshotted) run that pauses on an
// elicit step whose schema has string/integer/boolean/enum fields with one
// REQUIRED field. The prior llm step is MOCKED (empty object) so the form
// renders empty + deterministic. Real-LLM tier (the run snapshots a model).

const HAS_ANTHROPIC = (process.env.ANTHROPIC_API_KEY ?? '').length > 0

const FORM_WORKFLOW_YAML = `$schema: "/schemas/2026-06-12/workflow-definition.schema.json"
inputs:
  - name: topic
    required: false
steps:
  - id: gen
    kind: llm
    prompt: "gen {{ inputs.topic }}"
    output_format: json
  - id: form
    kind: elicit
    message: "Fill the form"
    data: "{{ gen.output }}"
    schema:
      type: object
      properties:
        full_name:
          type: string
          title: "Full name"
        count:
          type: integer
          title: "Count"
        agree:
          type: boolean
          title: "Agree"
        color:
          type: string
          title: "Color"
          enum: ["red", "green", "blue"]
      required: [full_name]
    timeout_ms: 600000
    depends_on: [gen]
outputs:
  - name: out
    from: "{{ form.output }}"
    expose: full
`

test.describe('Workflows — elicit form field types + validation', () => {
  test.skip(!HAS_ANTHROPIC, 'ANTHROPIC_API_KEY not set — real-LLM E2E skipped')

  test('renders all schema field types and shows a validation error on empty required submit', async ({
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

    const workflowId = await seedDevWorkflow(request, apiURL, adminToken, 'e2e-elicit-form', FORM_WORKFLOW_YAML)

    // Mock the gen step with an empty object so the form fields render empty
    // (the required full_name is unfilled → submit triggers validation).
    const runResp = await request.post(`${apiURL}/api/workflows/${workflowId}/run`, {
      headers: { Authorization: `Bearer ${adminToken}` },
      data: { inputs: { topic: 'x' }, model_id: modelId, mocks: { gen: {} } },
    })
    expect(runResp.status(), `run should 202: ${await runResp.text()}`).toBe(202)

    await goToWorkflowsSettingsPage(page, baseURL)
    await openWorkflowCard(page, 'e2e-elicit-form')
    await byTestId(page, 'wf-detail-tabs-tab-runs').click()
    await expect(byTestId(page, 'wf-runs-list')).toBeVisible()
    await page.locator('[data-testid^="wf-run-source-tag-"]').first().click()

    // The elicit form renders.
    await expect(byTestId(page, 'wf-elicit-alert')).toBeVisible({ timeout: 15000 })

    // ea879 — every schema field type rendered (each by its type-specific testid:
    // string→input, integer→number, boolean→switch, enum→select).
    await expect(byTestId(page, 'wf-elicit-input-full_name')).toBeVisible()
    await expect(byTestId(page, 'wf-elicit-number-count')).toBeVisible()
    await expect(byTestId(page, 'wf-elicit-switch-agree')).toBeVisible()
    await expect(byTestId(page, 'wf-elicit-select-color')).toBeVisible()

    // fa60 — submitting with the required `full_name` empty surfaces the error.
    await byTestId(page, 'wf-elicit-submit-btn').click()
    await expect(byTestId(page, 'wf-elicit-error-alert')).toBeVisible({ timeout: 10000 })
  })
})
