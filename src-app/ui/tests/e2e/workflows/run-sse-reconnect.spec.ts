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
 * E2E — workflow run-progress SSE reconnection.
 *
 * `WorkflowRunProgressView` shows a "reconnecting…" warning while
 * `!run.connected && !terminal`; the run-progress SSE client
 * (`runProgressClient.ts`) fires `disconnected` (→ store `connected=false`) when
 * the stream drops and auto-reconnects with backoff. This drives the real path
 * by toggling the browser offline mid-run: the client loses the stream (shows
 * "reconnecting…") and recovers when the network returns.
 *
 * Real-LLM tier: soft-skipped without ANTHROPIC_API_KEY.
 */

const ANTHROPIC_KEY = process.env.ANTHROPIC_API_KEY ?? ''

const RUN_WORKFLOW_YAML = `$schema: "/schemas/2026-06-12/workflow-definition.schema.json"
inputs:
  - name: topic
    description: "subject"
    required: true
steps:
  - id: summarize
    kind: llm
    message: "Summarizing {{ inputs.topic }}"
    prompt: |
      Write three sentences about "{{ inputs.topic }}".
outputs:
  - name: summary
    from: "{{ summarize.output }}"
    expose: full
`

test.describe('Workflows - run-progress SSE reconnect', () => {
  test.skip(
    ANTHROPIC_KEY.length === 0,
    'ANTHROPIC_API_KEY not set — real-LLM SSE reconnect E2E skipped',
  )

  test('going offline mid-run shows "reconnecting…" and recovers', async ({
    page,
    request,
    context,
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
      'e2e-sse-reconnect',
      RUN_WORKFLOW_YAML,
    )

    await goToWorkflowsSettingsPage(page, baseURL)
    await openWorkflowCard(page, 'e2e-sse-reconnect')

    await byTestId(page, 'wf-detail-run-btn').click()
    await expect(byTestId(page, 'wf-run-dialog')).toBeVisible({
      timeout: 10000,
    })

    const topicField = byTestId(page, 'wf-run-input-topic')
    if (await topicField.count()) {
      await topicField.first().fill('the water cycle')
    } else {
      await byTestId(page, 'wf-run-json-textarea').fill(
        '{ "topic": "the water cycle" }',
      )
    }
    await byTestId(page, 'wf-run-model-select').click()
    await page
      .locator('[data-testid^="wf-run-model-select-opt-"]')
      .first()
      .click()
    await byTestId(page, 'wf-run-submit-btn').click()

    // The progress view mounts + the SSE stream connects.
    await expect(byTestId(page, 'wf-progress-status-tag')).toBeVisible({
      timeout: 15000,
    })

    // Drop the network mid-run → the client loses its stream and the view shows
    // the "reconnecting…" warning (run is still non-terminal on the client).
    await context.setOffline(true)
    await expect(byTestId(page, 'wf-progress-reconnecting')).toBeVisible({
      timeout: 20000,
    })

    // Restore the network → the client reconnects, the warning clears, and the
    // run reaches a terminal status.
    await context.setOffline(false)
    await expect(byTestId(page, 'wf-progress-reconnecting')).toHaveCount(0, {
      timeout: 30000,
    })
    await expect(byTestId(page, 'wf-progress-status-tag')).toContainText(
      'completed',
      { timeout: 60000 },
    )
  })
})
