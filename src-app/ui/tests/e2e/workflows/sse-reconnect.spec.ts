import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import {
  goToWorkflowsSettingsPage,
  openWorkflowCard,
  seedDevWorkflow,
} from './helpers/workflow-helpers'
import { byTestId } from '../testid'

/**
 * E2E — the run-progress SSE reconnect warning (audit gap all-5f31d86f6abd).
 *
 * `WorkflowRunProgressView.tsx:225-228` renders a "reconnecting…" warning
 * whenever `!run.connected && !terminal` — i.e. the per-run SSE stream
 * (`GET /api/workflow-runs/{id}/events`, opened by
 * `runProgressClient.subscribeRunProgress`) is down for a still-live run.
 * The store flips `connected=false` from the client's `.catch → disconnected`
 * handler (`runProgressClient.ts:139-143`), which then reconnects with backoff.
 * No spec ever exercised this connection-state UI.
 *
 * Deterministic, no LLM: a single bounded `elicit` step (no `llm` step → no
 * model) starts a run that durably pauses (non-terminal), so the warning's
 * `!terminal` guard holds. We drop the SSE by aborting its endpoint at the
 * network boundary (the ONLY thing mocked), assert the warning surfaces and
 * STAYS up across reconnect attempts, then restore the endpoint + remount the
 * view and assert the warning clears once the stream genuinely reconnects
 * (the live elicitation form — delivered only over a healthy SSE — appears).
 */

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

// Matches the per-run SSE stream regardless of api host/port.
const SSE_GLOB = '**/api/workflow-runs/*/events'

test.describe('Workflows - run-progress SSE reconnect', () => {
  test('"reconnecting…" warning shows while the run SSE is down and clears on reconnect', async ({
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
      'e2e-sse-reconnect',
      ELICIT_WORKFLOW_YAML,
    )

    // Start the run — it durably pauses on the bounded elicit gate, staying
    // non-terminal so the reconnect warning's `!terminal` guard holds.
    const runResp = await request.post(
      `${apiURL}/api/workflows/${workflowId}/run`,
      {
        headers: { Authorization: `Bearer ${adminToken}` },
        data: { inputs: { topic: 'shipping the feature' } },
      },
    )
    expect(runResp.status(), `run should 202: ${await runResp.text()}`).toBe(202)

    // --- SSE DOWN: abort every run-events stream attempt (network boundary). ---
    await page.route(SSE_GLOB, route => route.abort())

    await goToWorkflowsSettingsPage(page, baseURL)
    await openWorkflowCard(page, 'e2e-sse-reconnect')
    await expect(byTestId(page, 'wf-runs-list')).toBeVisible()
    await page.locator('[data-testid^="wf-run-source-tag-"]').first().click()

    // The run-progress view mounts + subscribes; the SSE can't connect, so
    // `connected` never flips true and the warning surfaces for the live run.
    const reconnecting = byTestId(page, 'wf-progress-reconnecting')
    await expect(reconnecting).toBeVisible({ timeout: 15000 })

    // It must PERSIST — every backoff reconnect is aborted too, so this is the
    // real failing-reconnect loop, not a one-frame initial flash.
    await page.waitForTimeout(4000)
    await expect(reconnecting).toBeVisible()
    // And the live elicitation form never arrived (it rides the dead stream).
    await expect(byTestId(page, 'wf-elicit-alert')).toHaveCount(0)

    // --- SSE RESTORED: let the stream through and remount the view. ---
    await page.unroute(SSE_GLOB)
    // Navigate away (unmount → unsubscribe) then back (remount → fresh
    // subscribe), so the run-progress view opens a brand-new, now-reachable
    // SSE stream.
    await goToWorkflowsSettingsPage(page, baseURL)
    await openWorkflowCard(page, 'e2e-sse-reconnect')
    await expect(byTestId(page, 'wf-runs-list')).toBeVisible()
    await page.locator('[data-testid^="wf-run-source-tag-"]').first().click()

    // A healthy SSE delivers the snapshot + elicitation frame → `connected`
    // flips true → the warning clears, and the live form (proof the stream is
    // genuinely connected, not just silent) renders.
    await expect(byTestId(page, 'wf-elicit-alert')).toBeVisible({
      timeout: 15000,
    })
    await expect(byTestId(page, 'wf-progress-reconnecting')).toHaveCount(0)
  })
})
