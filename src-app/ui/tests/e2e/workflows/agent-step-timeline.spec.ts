import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'
import {
  goToWorkflowsSettingsPage,
  openWorkflowCard,
  seedDevWorkflow,
} from './helpers/workflow-helpers'
import { byTestId } from '../testid'

/**
 * TEST-20 — running a workflow that contains an AGENT step renders the friendly
 * activity TIMELINE (ITEM-5/ITEM-9): accreting domain-language rows with status
 * pills + a "Show details" disclosure — not a single collapsing log line.
 *
 * Real-LLM tier. Point a real OpenAI-compatible endpoint via env (mirrors the
 * `run-js-real-llm` bridge seam):
 *   OPENAI_BASE_URL      e.g. http://localhost:4000/v1
 *   OPENAI_API_KEY       the bridge key
 *   ZIEE_TEST_LLM_MODEL  the served model id (e.g. qwen3.6-35b-a3b)
 * Skips cleanly when unset. No API mocking — the agent loop, the SSE activity
 * stream, and the timeline all run for real.
 */

const BRIDGE = process.env.OPENAI_BASE_URL
const KEY = process.env.OPENAI_API_KEY
const MODEL = process.env.ZIEE_TEST_LLM_MODEL || process.env.OPENAI_MODEL

// A minimal single agent-step workflow: a reasoning-only task (no tools) that
// still drives a real agent turn → at least one Message activity row on the
// timeline. Kept tiny so the real-LLM spend is negligible.
const AGENT_WORKFLOW_YAML = `$schema: "/schemas/2026-06-12/workflow-definition.schema.json"
steps:
  - id: assist
    kind: agent
    description: "Assistant task"
    prompt: |
      Reply with exactly the single word: DONE
    max_steps: 3
    output_format: text
outputs:
  - name: answer
    from: "{{ assist.output }}"
    expose: full
`

test.describe('Workflows — agent step activity timeline (real LLM)', () => {
  test.skip(
    !BRIDGE || !KEY || !MODEL,
    'no real LLM endpoint (set OPENAI_BASE_URL + OPENAI_API_KEY + ZIEE_TEST_LLM_MODEL)',
  )
  test.setTimeout(180_000)

  test('run an agent workflow → the friendly activity timeline accretes rows', async ({
    page,
    request,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra

    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)

    // Real provider pointing at the bridge, granted to admins.
    const providerId = await createProviderViaAPI(
      apiURL,
      adminToken,
      'Bridge',
      'openai',
    )
    await assignProviderToAdministratorsGroup(apiURL, adminToken, providerId)

    // The agent step needs a TOOL-CAPABLE model (capabilities.tools=true).
    // createModelViaAPI does not set `tools`, so POST the model directly.
    const modelRes = await request.post(`${apiURL}/api/llm-models`, {
      headers: { Authorization: `Bearer ${adminToken}` },
      data: {
        provider_id: providerId,
        name: MODEL,
        display_name: 'Bridge Agent Model',
        enabled: true,
        engine_type: 'none',
        file_format: 'gguf',
        capabilities: { tools: true, chat: true, streaming: true },
        parameters: {
          context_length: 8192,
          temperature: 0,
          top_p: 0.9,
          max_tokens: 512,
        },
      },
    })
    expect(modelRes.ok(), `create model: ${await modelRes.text()}`).toBeTruthy()

    // Seed the agent workflow via the real import API.
    await seedDevWorkflow(
      request,
      apiURL,
      adminToken,
      'e2e-agent-timeline',
      AGENT_WORKFLOW_YAML,
    )

    // Open it and start a run with the bridge model.
    await goToWorkflowsSettingsPage(page, baseURL)
    await openWorkflowCard(page, 'e2e-agent-timeline')
    await byTestId(page, 'wf-detail-run-btn').click()
    await expect(byTestId(page, 'wf-run-dialog')).toBeVisible({ timeout: 10000 })

    // Pick the (only) model.
    await byTestId(page, 'wf-run-model-select').click()
    await page
      .locator('[data-testid^="wf-run-model-select-opt-"]')
      .first()
      .click()

    await byTestId(page, 'wf-run-submit-btn').click()

    // The friendly ACTIVITY TIMELINE renders for the agent step: at least one
    // accreting activity row with a status pill.
    const timeline = byTestId(page, 'wf-activity-timeline-assist')
    await expect(timeline).toBeVisible({ timeout: 90_000 })

    const activityRow = page
      .locator('[data-testid^="wf-activity-row-assist-"]')
      .first()
    await expect(activityRow).toBeVisible({ timeout: 90_000 })
    // The row carries a status pill (Running / Done / Error).
    await expect(
      page.locator('[data-testid^="wf-activity-status-assist-"]').first(),
    ).toBeVisible()
    // The "Show details" disclosure is present on a detailed row.
    await expect(timeline.getByText('Show details').first()).toBeVisible({
      timeout: 30_000,
    })

    // The run completes.
    await expect(byTestId(page, 'wf-progress-status-tag')).toContainText(
      'completed',
      { timeout: 120_000 },
    )
  })
})
