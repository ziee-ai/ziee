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
  })
})
