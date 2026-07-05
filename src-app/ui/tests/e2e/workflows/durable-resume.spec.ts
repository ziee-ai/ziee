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
 * Durable workflow resume (Change B) through the real UI.
 *
 * A `timeout_ms: 0` elicit gate is a DURABLE checkpoint: the runner persists
 * the pending form, flips the run to `waiting`, and SUSPENDS (the task exits —
 * no resident runner). The run row + its outputs are the durable checkpoint.
 *
 * This drives the cold path end-to-end:
 *   seed a 2-step workflow (`screen` llm mocked → `gate` elicit, timeout_ms:0)
 *   → start the run (run SUSPENDS on the gate, status `waiting`) → open it →
 *   the form renders FROM THE DB SNAPSHOT (no resident runner) → RELOAD the
 *   page → the form re-renders from the snapshot again (durable reconnect) →
 *   Submit → `submit_elicit` finds no resident handle, persists the response,
 *   spawns `resume_run` (cold resume), which skips the completed `screen` step
 *   and consumes the response → the run completes.
 *
 * Real-LLM tier only because the run snapshots a model at start (the `screen`
 * step is mocked, so no token is actually spent). Backend coverage:
 * `tests/workflow/resume.rs`.
 */

const ANTHROPIC_KEY = process.env.ANTHROPIC_API_KEY ?? ''
const HAS_ANTHROPIC = ANTHROPIC_KEY.length > 0

// `screen` (llm, mocked, json) → `gate` (elicit, DURABLE timeout_ms:0).
const DURABLE_WORKFLOW_YAML = `$schema: "/schemas/2026-06-12/workflow-definition.schema.json"
inputs:
  - name: topic
    required: false
steps:
  - id: screen
    kind: llm
    prompt: "screen {{ inputs.topic }}"
    output_format: json
  - id: gate
    kind: elicit
    message: "Approve to continue"
    schema:
      type: object
      properties:
        approved:
          type: boolean
          title: "Approve?"
      required: [approved]
    timeout_ms: 0
    depends_on: [screen]
outputs:
  - name: decision
    from: "{{ gate.output }}"
    expose: full
`

test.describe('Workflows - durable resume (timeout_ms:0 suspend → cold resume)', () => {
  test.skip(!HAS_ANTHROPIC, 'ANTHROPIC_API_KEY not set — real-LLM E2E skipped')

  test('suspend on durable gate → reload re-renders form → submit cold-resumes → completed', async ({
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
      'e2e-durable-resume',
      DURABLE_WORKFLOW_YAML,
    )

    // Start the run: mock `screen`; the run SUSPENDS on the durable gate.
    const runResp = await request.post(
      `${apiURL}/api/workflows/${workflowId}/run`,
      {
        headers: { Authorization: `Bearer ${adminToken}` },
        data: {
          inputs: { topic: 'x' },
          model_id: modelId,
          mocks: { screen: { ok: true } },
        },
      },
    )
    expect(runResp.status(), `run should 202: ${await runResp.text()}`).toBe(202)

    // Open the workflow → its (suspended) run.
    await goToWorkflowsSettingsPage(page, baseURL)
    await openWorkflowCard(page, 'e2e-durable-resume')
    await byTestId(page, 'wf-detail-tabs-tab-runs').click()
    await expect(byTestId(page, 'wf-runs-list')).toBeVisible()
    await page.locator('[data-testid^="wf-run-source-tag-"]').first().click()

    // The form renders even though NO runner is resident (served from the DB
    // snapshot). The run-level status reads `waiting` (the durable, non-terminal
    // state introduced by Change B).
    await expect(byTestId(page, 'wf-elicit-alert')).toBeVisible({
      timeout: 15000,
    })
    await expect(byTestId(page, 'wf-progress-status-tag')).toContainText(
      'waiting',
      { timeout: 10000 },
    )

    // RELOAD: a fresh page re-subscribes and rebuilds the pending form purely
    // from the persisted snapshot — the durable, cross-reconnect property.
    await page.reload()
    await goToWorkflowsSettingsPage(page, baseURL)
    await openWorkflowCard(page, 'e2e-durable-resume')
    // The detail drawer re-opens on the Details tab; switch to Runs first.
    await byTestId(page, 'wf-detail-tabs-tab-runs').click()
    await expect(byTestId(page, 'wf-runs-list')).toBeVisible()
    await page.locator('[data-testid^="wf-run-source-tag-"]').first().click()
    await expect(byTestId(page, 'wf-elicit-alert')).toBeVisible({
      timeout: 15000,
    })

    // Submit → cold resume (no resident runner): persists the response, spawns
    // resume_run, which skips `screen` and consumes the gate → run completes.
    const gateSwitch = page.getByRole('switch').first()
    if (await gateSwitch.count()) {
      await gateSwitch.click()
    }
    await byTestId(page, 'wf-elicit-submit-btn').click()

    await expect(byTestId(page, 'wf-progress-status-tag')).toContainText(
      'completed',
      { timeout: 30000 },
    )
  })
})
