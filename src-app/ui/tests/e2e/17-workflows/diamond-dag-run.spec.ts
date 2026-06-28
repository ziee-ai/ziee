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
 * Multi-step parallel/diamond DAG through the UI run-progress view. The backend
 * topo-sort/fan-in is covered by tests/workflow/diamond_dag.rs; this drives a
 * diamond (A → {B,C} → D) run and asserts the WorkflowRunProgressView renders
 * every step to completion. All llm steps are MOCKED (no tokens); the run still
 * snapshots a model, so it's ANTHROPIC-gated.
 */
const ANTHROPIC_KEY = process.env.ANTHROPIC_API_KEY ?? ''
const HAS_ANTHROPIC = ANTHROPIC_KEY.length > 0

const DIAMOND_YAML = `$schema: "/schemas/2026-06-12/workflow-definition.schema.json"
inputs:
  - name: topic
    required: false
steps:
  - id: a
    kind: llm
    prompt: "seed {{ inputs.topic }}"
  - id: b
    kind: llm
    prompt: "branch B over {{ a.output }}"
    depends_on: [a]
  - id: c
    kind: llm
    prompt: "branch C over {{ a.output }}"
    depends_on: [a]
  - id: d
    kind: llm
    prompt: "join {{ b.output }} and {{ c.output }}"
    depends_on: [b, c]
outputs:
  - name: combined
    from: "{{ b.output }}+{{ c.output }}"
    expose: full
`

test.describe('Workflows - diamond DAG run (real LLM snapshot)', () => {
  test.skip(!HAS_ANTHROPIC, 'ANTHROPIC_API_KEY not set — real-LLM E2E skipped')
  test.slow()

  test('a fan-out/fan-in diamond runs every step to completion', async ({
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
      'e2e-diamond',
      DIAMOND_YAML,
    )

    // All four llm steps mocked → deterministic, no tokens spent.
    const runResp = await request.post(
      `${apiURL}/api/workflows/${workflowId}/run`,
      {
        headers: { Authorization: `Bearer ${adminToken}` },
        data: {
          inputs: { topic: 'x' },
          model_id: modelId,
          mocks: { a: 'SEED', b: 'B_RESULT', c: 'C_RESULT', d: 'D_DONE' },
        },
      },
    )
    expect(runResp.status(), `run should 202: ${await runResp.text()}`).toBe(
      202,
    )

    await goToWorkflowsSettingsPage(page, baseURL)
    await openWorkflowCard(page, 'e2e-diamond')
    await expect(page.getByText('Runs', { exact: true })).toBeVisible()
    await page.getByText('Workflow page', { exact: true }).first().click()

    // The run-progress view lists every diamond step and the run reaches
    // completed (fan-out B/C + fan-in D all resolved).
    await expect(
      page.getByText('completed', { exact: true }).first(),
    ).toBeVisible({ timeout: 45000 })
  })
})
