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

    await page.getByRole('button', { name: /Run$/ }).first().click()
    await expect(page.getByRole('dialog', { name: /^Run / })).toBeVisible({
      timeout: 10000,
    })

    const topicField = page.getByLabel('topic')
    if (await topicField.count()) {
      await topicField.first().fill('the water cycle')
    } else {
      await page.getByPlaceholder(/"topic"/).fill('{ "topic": "the water cycle" }')
    }
    const modelSelect = page.getByLabel('Model')
    await modelSelect.click()
    await page.getByRole('option', { name: /Claude Haiku 4\.5/ }).first().click()
    await page.getByRole('button', { name: 'Run', exact: true }).last().click()

    // The progress view mounts + the SSE stream connects.
    await expect(page.getByText('Run progress')).toBeVisible({ timeout: 15000 })

    // Drop the network mid-run → the client loses its stream and the view shows
    // the "reconnecting…" warning (run is still non-terminal on the client).
    await context.setOffline(true)
    await expect(page.getByText('reconnecting…')).toBeVisible({ timeout: 20000 })

    // Restore the network → the client reconnects, the warning clears, and the
    // run reaches a terminal status.
    await context.setOffline(false)
    await expect(page.getByText('reconnecting…')).toHaveCount(0, {
      timeout: 30000,
    })
    await expect(
      page.getByText('completed', { exact: true }).first(),
    ).toBeVisible({ timeout: 60000 })
  })
})
