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
 * Run cancellation — the Cancel button on WorkflowRunProgressView
 * (WorkflowRunProgressView.tsx:230-239 → WorkflowRun.store cancel()).
 *
 * A `screen` (llm, MOCKED → no tokens) → `review` (elicit, long timeout)
 * workflow pauses at the elicit step in a non-terminal `waiting` state. Rather
 * than submitting, we click Cancel and assert the run transitions to
 * `cancelled` (POST /api/workflows/runs/{id}/cancel through the real handler).
 *
 * Real-LLM tier only because the run still snapshots a model at start; the
 * screening output is mocked, so behaviour is deterministic. Backend coverage
 * of the cancel transition: tests/workflow/status_machine.rs.
 */

const ANTHROPIC_KEY = process.env.ANTHROPIC_API_KEY ?? ''
const HAS_ANTHROPIC = ANTHROPIC_KEY.length > 0

const CANCEL_WORKFLOW_YAML = `$schema: "/schemas/2026-06-12/workflow-definition.schema.json"
inputs:
  - name: topic
    required: false
steps:
  - id: screen
    kind: llm
    prompt: "screen {{ inputs.topic }}"
    output_format: json
  - id: review
    kind: elicit
    message: "Review before continuing"
    data: "{{ screen.output }}"
    schema:
      type: object
      properties:
        note:
          type: string
      required: [note]
    timeout_ms: 600000
    depends_on: [screen]
outputs:
  - name: decision
    from: "{{ review.output }}"
    expose: full
`

test.describe('Workflows - run cancellation (real LLM snapshot)', () => {
  test.skip(!HAS_ANTHROPIC, 'ANTHROPIC_API_KEY not set — real-LLM E2E skipped')

  test('Cancel on a paused run transitions it to cancelled', async ({
    page,
    request,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra

    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)

    const providerId = await createProviderViaAPI(
      apiURL,
      adminToken,
      'Anthropic',
      'anthropic',
    )
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
      'e2e-cancel-run',
      CANCEL_WORKFLOW_YAML,
    )

    // Start the run; mock `screen` (deterministic, no tokens). It pauses on the
    // `review` elicit step in a non-terminal state.
    const runResp = await request.post(
      `${apiURL}/api/workflows/${workflowId}/run`,
      {
        headers: { Authorization: `Bearer ${adminToken}` },
        data: {
          inputs: { topic: 'x' },
          model_id: modelId,
          mocks: { screen: { note: 'pending' } },
        },
      },
    )
    expect(runResp.status(), `run should 202: ${await runResp.text()}`).toBe(202)

    await goToWorkflowsSettingsPage(page, baseURL)
    await openWorkflowCard(page, 'e2e-cancel-run')

    await byTestId(page, 'wf-detail-tabs-tab-runs').click()
    await expect(byTestId(page, 'wf-runs-list')).toBeVisible()
    await page.locator('[data-testid^="wf-run-source-tag-"]').first().click()

    // The run is paused (elicit "Input required"); the Cancel button is shown
    // because the run is non-terminal.
    await expect(byTestId(page, 'wf-elicit-alert')).toBeVisible({
      timeout: 15000,
    })
    const cancelBtn = byTestId(page, 'wf-progress-cancel-btn')
    await expect(cancelBtn).toBeVisible()
    await cancelBtn.click()

    // The run-level status Tag flips to "cancelled".
    await expect(byTestId(page, 'wf-progress-status-tag')).toContainText(
      'cancelled',
      { timeout: 30000 },
    )
  })
})
