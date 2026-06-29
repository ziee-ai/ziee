import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import {
  goToWorkflowsSettingsPage,
  openWorkflowCard,
  seedDevWorkflow,
} from './helpers/workflow-helpers'
import { byTestId } from '../testid'

/**
 * E2E — the "Remove timeout" button on the live run progress view
 * (WorkflowRunProgressView.tsx:164-186). Audit gap all-21d11894c9f9: the
 * button calls `ApiClient.Workflow.setRunTimeout({ timeout_secs: 0 })` to
 * lift a running run's wall-clock cap, and no spec ever clicked it.
 *
 * Deterministic, no LLM: a single `kind: elicit` step (no upstream `llm`
 * step) runs WITHOUT a model_id and pauses on the gate with a RESIDENT
 * runner (a live deadline watcher, because `timeout_ms > 0`). Because the
 * runner is resident, `registry::set_timeout` returns `true` → the handler
 * acks `status: "updated"` → the UI shows the "Timeout removed" success
 * toast (the branch a durable `timeout_ms: 0` / non-resident run would NOT
 * reach). We assert the real `PUT /api/workflow-runs/{id}/timeout` fires
 * and the success toast renders — the button genuinely lifts the cap.
 */

// Single bounded-timeout elicit gate, no `llm` step → no model needed, and
// the runner stays resident (live timer) so "Remove timeout" hits the
// `updated` success path.
const ELICIT_WORKFLOW_YAML = `$schema: "/schemas/2026-06-12/workflow-definition.schema.json"
inputs:
  - name: topic
    required: false
steps:
  - id: gate
    kind: elicit
    message: "Proceed with {{ inputs.topic }}?"
    schema:
      type: object
      properties:
        approved:
          type: boolean
          title: "Approve?"
      required: [approved]
    timeout_ms: 300000
outputs:
  - name: decision
    from: "{{ gate.output }}"
    expose: full
`

test.describe('Workflows - Remove timeout button', () => {
  test('clicking "Remove timeout" lifts the wall-clock cap on a live run', async ({
    page,
    request,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra

    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)

    const workflowId = await seedDevWorkflow(
      request,
      apiURL,
      adminToken,
      'e2e-remove-timeout',
      ELICIT_WORKFLOW_YAML,
    )

    // Start the run (no model_id — there is no llm step). It pauses on the
    // bounded elicit gate with a resident runner (live deadline watcher).
    const runResp = await request.post(
      `${apiURL}/api/workflows/${workflowId}/run`,
      {
        headers: { Authorization: `Bearer ${adminToken}` },
        data: { inputs: { topic: 'shipping the feature' } },
      },
    )
    expect(runResp.status(), `run should 202: ${await runResp.text()}`).toBe(202)
    const runId = (await runResp.json()).run_id as string

    // Open the workflow → its (paused) run's progress view.
    await goToWorkflowsSettingsPage(page, baseURL)
    await openWorkflowCard(page, 'e2e-remove-timeout')
    await expect(byTestId(page, 'wf-runs-list')).toBeVisible()
    await page.locator('[data-testid^="wf-run-source-tag-"]').first().click()

    // The pending elicitation form proves the run is live + non-terminal,
    // so the "Remove timeout" button (only shown while !terminal) renders.
    await expect(byTestId(page, 'wf-elicit-alert')).toBeVisible({
      timeout: 15000,
    })

    const removeBtn = byTestId(page, 'wf-progress-remove-timeout-btn')
    await expect(removeBtn).toBeVisible({ timeout: 10000 })

    // Click it and assert the REAL setRunTimeout call fires for THIS run
    // (PUT /api/workflow-runs/{run_id}/timeout) with timeout_secs: 0.
    const [timeoutReq] = await Promise.all([
      page.waitForResponse(
        resp =>
          resp.url().includes(`/workflow-runs/${runId}/timeout`) &&
          resp.request().method() === 'PUT',
        { timeout: 15000 },
      ),
      removeBtn.click(),
    ])
    expect(timeoutReq.status()).toBe(200)
    expect(JSON.parse(timeoutReq.request().postData() ?? '{}')).toMatchObject({
      timeout_secs: 0,
    })
    // Resident run → the registry update applied → "updated" ack → success toast.
    expect((await timeoutReq.json()).status).toBe('updated')

    // The UI surfaces the success message ("Timeout removed — this run is no
    // longer wall-clock limited"), confirming the button drove the cap-lift.
    await expect(
      page.locator('[data-sonner-toast][data-type="success"]'),
    ).toContainText('Timeout removed', { timeout: 10000 })
  })
})
