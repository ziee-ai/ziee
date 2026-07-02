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
 * A4/A5 — the Runs tab: a completed run lists with its invocation-source badge
 * ("Workflow page"), opens into the run-progress view when clicked, and is
 * deletable (Popconfirm) from the list.
 *
 * Drives the full UI flow:
 *   login → real provider/model → seed a 1-step `llm` workflow → run it once
 *   from the dialog → the "Runs" section lists the run with the "Workflow page"
 *   badge → click it (progress view) → delete it (Popconfirm).
 *
 * Real-LLM tier (needs a model to produce a real run): gated on
 * ANTHROPIC_API_KEY. Backend coverage of the list/delete contracts is
 * `tests/workflow/run_history_and_delete.rs`.
 */

const ANTHROPIC_KEY = process.env.ANTHROPIC_API_KEY ?? ''
const HAS_ANTHROPIC = ANTHROPIC_KEY.length > 0

const HISTORY_WORKFLOW_YAML = `$schema: "/schemas/2026-06-12/workflow-definition.schema.json"
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

test.describe('Workflows - run history (Runs tab) (real LLM)', () => {
  test.skip(!HAS_ANTHROPIC, 'ANTHROPIC_API_KEY not set — real-LLM E2E skipped')

  test('a run appears in the Runs tab with a source badge, opens, and deletes', async ({
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
    await createModelViaAPI(
      apiURL,
      adminToken,
      providerId,
      'claude-haiku-4-5-20251001',
      'Claude Haiku 4.5',
      'anthropic',
    )

    await seedDevWorkflow(
      request,
      apiURL,
      adminToken,
      'e2e-history-wf',
      HISTORY_WORKFLOW_YAML,
    )

    await goToWorkflowsSettingsPage(page, baseURL)
    await openWorkflowCard(page, 'e2e-history-wf')

    // Run once. The drawer Run button has a PlayCircle icon → accessible name
    // "play-circle Run"; match the trailing "Run" (the dialog OK button below
    // is a plain exact "Run").
    await byTestId(page, 'wf-detail-run-btn').click()
    // The Run dialog (titled "Run <workflow>") opened — target it by testid.
    await expect(byTestId(page, 'wf-run-dialog')).toBeVisible({
      timeout: 10000,
    })
    const topicField = byTestId(page, 'wf-run-input-topic')
    if (await topicField.count()) {
      await topicField.first().fill('photosynthesis')
    } else {
      await byTestId(page, 'wf-run-json-textarea').fill(
        '{ "topic": "photosynthesis" }',
      )
    }
    await byTestId(page, 'wf-run-model-select').click()
    await page
      .locator('[data-testid^="wf-run-model-select-opt-"]')
      .first()
      .click()
    await byTestId(page, 'wf-run-submit-btn').click()

    // Wait for the run to complete (the run-level status tag reads "completed").
    await expect(byTestId(page, 'wf-progress-status-tag')).toContainText(
      'completed',
      { timeout: 60000 },
    )

    // The Runs section lists the run with the "Workflow page" source badge.
    await byTestId(page, 'wf-detail-tabs-tab-runs').click()
    await expect(byTestId(page, 'wf-runs-list')).toBeVisible()
    const sourceTag = page.locator('[data-testid^="wf-run-source-tag-"]')
    await expect(sourceTag.first()).toBeVisible({ timeout: 15000 })
    await expect(sourceTag.first()).toContainText('Workflow page')

    // Click the run row → the run-progress view shows it.
    await sourceTag.first().click()
    await expect(byTestId(page, 'wf-progress-status-tag')).toBeVisible({
      timeout: 10000,
    })

    // Delete the run from the Runs list → Confirm → confirm. The per-row delete
    // is an icon-only Button; the workflow-level delete lives in the drawer
    // footer with a distinct testid, so this only targets the run row's delete.
    await page.locator('[data-testid^="wf-run-delete-btn-"]').first().click()
    // Confirm popover: "Delete this run?" with an OK button labeled "Delete".
    await expect(
      page.locator('[data-testid^="wf-run-delete-confirm-"]').first(),
    ).toBeVisible()
    await page
      .locator('[data-testid^="wf-run-delete-confirm-"][data-testid$="-confirm"]')
      .first()
      .click()

    // The run row is gone from the list.
    await expect(sourceTag).toHaveCount(0, { timeout: 15000 })
  })

  test('cancelling the delete Popconfirm keeps the run in the list', async ({
    page,
    request,
    testInfra,
  }) => {
    // Edge case not covered by the happy-path test: the delete-Popconfirm
    // CANCEL branch must NOT remove the run. The `summarize` step is mocked via
    // the run API (no token spent; model only snapshotted) so the run completes
    // deterministically.
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
      'e2e-history-cancel-wf',
      HISTORY_WORKFLOW_YAML,
    )

    const runResp = await request.post(`${apiURL}/api/workflows/${workflowId}/run`, {
      headers: { Authorization: `Bearer ${adminToken}` },
      data: {
        inputs: { topic: 'photosynthesis' },
        model_id: modelId,
        mocks: { summarize: 'A one-line deterministic summary.' },
      },
    })
    expect(runResp.status(), `run should 202: ${await runResp.text()}`).toBe(202)

    await goToWorkflowsSettingsPage(page, baseURL)
    await openWorkflowCard(page, 'e2e-history-cancel-wf')

    // The run lists in the Runs tab.
    await byTestId(page, 'wf-detail-tabs-tab-runs').click()
    await expect(byTestId(page, 'wf-runs-list')).toBeVisible()
    const sourceTag = page.locator('[data-testid^="wf-run-source-tag-"]')
    await expect(sourceTag.first()).toBeVisible({ timeout: 15000 })

    // Open the per-row delete Confirm, then CANCEL it.
    await page.locator('[data-testid^="wf-run-delete-btn-"]').first().click()
    const confirm = page.locator('[data-testid^="wf-run-delete-confirm-"]')
    await expect(confirm.first()).toBeVisible()
    await page
      .locator('[data-testid^="wf-run-delete-confirm-"][data-testid$="-cancel"]')
      .first()
      .click()

    // The run row is still present (cancel did not delete it).
    await expect(
      page.locator('[data-testid^="wf-run-delete-confirm-"][data-testid$="-cancel"]'),
    ).toHaveCount(0, { timeout: 10000 })
    await expect(sourceTag.first()).toBeVisible()
  })
})
