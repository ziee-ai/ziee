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
} from '../17-workflows/helpers/workflow-helpers'

/**
 * E2E (sync) — a workflow run's state transition surfaces in the Runs UI
 * LIVE via the `sync:workflow_run` channel (WorkflowRuns.store.ts:46
 * `eventBus.on('sync:workflow_run', reload)`), with NO manual reload.
 *
 * Audit gap: 13-sync/ had no workflow_run coverage. This loads a workflow's
 * (empty) Runs list — registering it for the store's sync reload — then
 * triggers a run via the API with the single llm step MOCKED (deterministic,
 * no real-LLM spend). The backend emits `sync:workflow_run` on the run's
 * state transition (origin=None), and the Runs list must populate with the
 * completed run without the test ever calling page.reload().
 */

const RUN_WORKFLOW_YAML = `$schema: "/schemas/2026-06-12/workflow-definition.schema.json"
inputs:
  - name: topic
    required: true
steps:
  - id: summarize
    kind: llm
    message: "Summarizing {{ inputs.topic }}"
    prompt: |
      In ONE short sentence, say something about "{{ inputs.topic }}".
outputs:
  - name: summary
    from: "{{ summarize.output }}"
    expose: full
`

test.describe('Sync — workflow run state via sync:workflow_run', () => {
  test('a mocked run appears in the Runs list live (no reload)', async ({
    page,
    request,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)

    // A model is required (snapshotted onto the run) — the step is mocked, so
    // the provider key is never used.
 * Realtime sync for workflow runs (WorkflowRuns.store subscribes to
 * `sync:workflow_run` and reloads). A run started + transitioned on one device
 * reaches the SAME user's other device live — including the mid-run
 * elicitation pause and the terminal cancel — with NO manual reload.
 *
 * Real-LLM tier: the run snapshots a model at start (the `screen` step is
 * mocked, so no tokens spent). Gated on ANTHROPIC_API_KEY. --workers=1.
 */
const ANTHROPIC_KEY = process.env.ANTHROPIC_API_KEY ?? ''
const HAS_ANTHROPIC = ANTHROPIC_KEY.length > 0

const WF_YAML = `$schema: "/schemas/2026-06-12/workflow-definition.schema.json"
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

test.describe('Realtime sync — workflow run transitions', () => {
  test.skip(!HAS_ANTHROPIC, 'ANTHROPIC_API_KEY not set — real-LLM E2E skipped')
  test.slow()

  test('a run started on device A (paused at elicit, then cancelled) updates device B live', async ({
    page,
    request,
    browser,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra

    // Setup: provider + model + a seeded elicit workflow (device A).
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
      undefined,
      undefined,
      'anthropic',
    )

      'claude-haiku-4-5-20251001',
      'Claude Haiku 4.5',
      'anthropic',
    )
    const workflowId = await seedDevWorkflow(
      request,
      apiURL,
      adminToken,
      'e2e-run-sync',
      RUN_WORKFLOW_YAML,
    )

    // Open the workflow card → its Runs list loads (empty), which registers
    // this workflow id in the WorkflowRuns store so the sync reload targets it.
    await goToWorkflowsSettingsPage(page, baseURL)
    await openWorkflowCard(page, 'e2e-run-sync')
    await expect(page.getByText('Runs', { exact: true })).toBeVisible()

    // Trigger the run via the API (a "background" mutation: origin=None) with
    // the llm step mocked → it completes deterministically.
    const runResp = await request.post(
      `${apiURL}/api/workflows/${workflowId}/run`,
      {
        headers: { Authorization: `Bearer ${adminToken}` },
        data: {
          inputs: { topic: 'x' },
          model_id: modelId,
          mocks: { summarize: 'A mocked one-sentence summary.' },
        },
      },
    )
    expect(runResp.status(), `run should 202: ${await runResp.text()}`).toBe(
      202,
    )

    // Without any page.reload(), the sync:workflow_run event drives the
    // WorkflowRuns store to refetch and the run surfaces in the list (its
    // invocation-source badge is "Workflow page").
    await expect(
      page.getByText('Workflow page', { exact: true }).first(),
    ).toBeVisible({ timeout: 30000 })
      WF_YAML,
    )

    // Device B: same user, open the workflow detail (its Runs list) BEFORE the
    // run starts so the live sync delivery is what surfaces it.
    const ctxB = await browser.newContext()
    const pageB = await ctxB.newPage()
    try {
      await loginAsAdmin(pageB, baseURL)
      await goToWorkflowsSettingsPage(pageB, baseURL)
      await openWorkflowCard(pageB, 'e2e-run-sync')
      await expect(pageB.getByText('Runs', { exact: true })).toBeVisible()

      // Device A starts the run (screen mocked → no tokens; pauses on elicit).
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
      expect(runResp.status(), `run should 202: ${await runResp.text()}`).toBe(
        202,
      )
      const runId: string = (await runResp.json()).run_id

      // Device B sees the new run appear live (sync:workflow_run → reload), no
      // manual reload.
      await expect(
        pageB.getByText('Workflow page', { exact: true }).first(),
      ).toBeVisible({ timeout: 20000 })

      // Device A cancels the run.
      const cancel = await request.post(
        `${apiURL}/api/workflow-runs/${runId}/cancel`,
        { headers: { Authorization: `Bearer ${adminToken}` } },
      )
      expect(cancel.ok()).toBe(true)

      // Device B reflects the terminal `cancelled` status live.
      await expect(
        pageB.getByText('cancelled', { exact: true }).first(),
      ).toBeVisible({ timeout: 20000 })
    } finally {
      await ctxB.close()
    }
  })
})
