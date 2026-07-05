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
 * A1/A7 — run a standalone workflow from the settings page with the model
 * picker + the "Capture debug logs" toggle, and watch progress stream to
 * completion in the run-progress view.
 *
 * Drives the full UI flow:
 *   login → configure Anthropic provider (real key) → seed a 1-step `llm`
 *   workflow via the API → /settings/workflows → open the card → Run → pick a
 *   model → toggle capture-logs → run → progress streams → completed.
 *
 * Real-LLM tier: gated on ANTHROPIC_API_KEY (soft-skip when unset), mirroring
 * `projects/message-uses-project-context.spec.ts`. The Rust-side coverage of
 * the same backend path is `tests/workflow/run_model.rs` (model_id → 202).
 */

const ANTHROPIC_KEY = process.env.ANTHROPIC_API_KEY ?? ''
const HAS_ANTHROPIC = ANTHROPIC_KEY.length > 0

// A tiny single-step llm workflow — keep the prompt + output tight so the
// real-LLM spend is negligible.
const RUN_WORKFLOW_YAML = `$schema: "/schemas/2026-06-12/workflow-definition.schema.json"
inputs:
  - name: topic
    description: "subject to summarize"
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

test.describe('Workflows - run a standalone workflow (real LLM)', () => {
  test.skip(!HAS_ANTHROPIC, 'ANTHROPIC_API_KEY not set — real-LLM E2E skipped')

  test('pick a model, capture logs, run → progress streams to completed', async ({
    page,
    request,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra

    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)

    // Real Anthropic provider + a fast Haiku model, granted to admins.
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

    // Seed a dev workflow via the API so the list has one to open.
    await seedDevWorkflow(
      request,
      apiURL,
      adminToken,
      'e2e-run-summarize',
      RUN_WORKFLOW_YAML,
    )

    await goToWorkflowsSettingsPage(page, baseURL)
    // The seeded workflow's slug becomes `local.dev/e2e-run-summarize`; the
    // card shows the name. Open it.
    await openWorkflowCard(page, 'e2e-run-summarize')

    // Click the drawer Run button → the Run dialog opens. The drawer button
    // has a PlayCircle icon, so its accessible name is "play-circle Run" (the
    // icon's aria-label is concatenated); match the trailing "Run" rather than
    // exact. The dialog's own OK button below is a plain "Run" (exact).
    await byTestId(page, 'wf-detail-run-btn').click()
    // Assert the Run dialog (titled "Run <workflow>") opened — target it by its
    // i18n-safe testid.
    await expect(byTestId(page, 'wf-run-dialog')).toBeVisible({
      timeout: 10000,
    })

    // Provide the required `topic` input. With structured inputs the field is
    // testid'd by the input name; fall back to the free-form JSON editor.
    const topicField = byTestId(page, 'wf-run-input-topic')
    if (await topicField.count()) {
      await topicField.first().fill('quantum entanglement')
    } else {
      await byTestId(page, 'wf-run-json-textarea').fill(
        '{ "topic": "quantum entanglement" }',
      )
    }

    // Pick a model in the standalone picker (only one model is registered).
    await byTestId(page, 'wf-run-model-select').click()
    await page
      .locator('[data-testid^="wf-run-model-select-opt-"]')
      .first()
      .click()

    // Turn on "Capture debug logs".
    const captureToggle = byTestId(page, 'wf-run-capture-logs-switch')
    await captureToggle.click()
    await expect(captureToggle).toBeChecked()

    // Run.
    await byTestId(page, 'wf-run-submit-btn').click()

    // The run-progress view appears and streams to completion. The status tag
    // transitions to "completed"; allow a generous budget for the real call.
    await expect(byTestId(page, 'wf-progress-status-tag')).toBeVisible({
      timeout: 15000,
    })
    await expect(byTestId(page, 'wf-progress-status-tag')).toContainText(
      'completed',
      { timeout: 60000 },
    )

    // A7: a completed LLM step surfaces its per-step log expanders — "Show
    // prompt" (an llm step has a prompt) and "Show trace" (trace.json is
    // written on completion). This guards the durable-log viewers and confirms
    // the trace affordance shows for a completed step (hidden for failed steps).
    const promptBtn = page.locator(
      '[data-testid^="wf-step-log-btn-"][data-testid$="-prompt"]',
    )
    const traceBtn = page.locator(
      '[data-testid^="wf-step-log-btn-"][data-testid$="-trace"]',
    )
    const promptAccordion = page
      .locator('[data-testid^="wf-step-log-accordion-"][data-testid$="-prompt"]')
      .first()
    const traceAccordion = page
      .locator('[data-testid^="wf-step-log-accordion-"][data-testid$="-trace"]')
      .first()
    const logUnavailable = page.locator('[data-testid="wf-step-log-empty"]')

    await expect(promptBtn.first()).toBeVisible({ timeout: 10000 })
    await expect(traceBtn.first()).toBeVisible()

    // A7 (expander interaction): clicking "Show prompt" lazily fetches the
    // captured prompt log and renders it inline — assert the rendered prompt
    // body appears (it embeds the step's prompt text). This exercises the
    // StepLogExpander fetch+expand path, not just the affordance's presence.
    await promptBtn.first().click()
    await expect(promptAccordion).toContainText(/say something about/i, {
      timeout: 15000,
    })
    // Expander INTERACTION (StepLogExpander): the prompt interpolates the
    // `topic` input, so the SAME rendered prompt body also contains
    // "quantum entanglement". The button is a toggle, so assert on the already-
    // open accordion rather than re-clicking (which would collapse it).
    await expect(promptAccordion).toContainText(/quantum entanglement/i, {
      timeout: 15000,
    })

    // "Show trace" lazily fetches + renders trace.json — its body is per-step
    // run metadata (started_at / ms_elapsed / attempts / …), NOT the step id,
    // so assert a real trace field to prove the fetch+render path ran.
    await traceBtn.first().click()
    await expect(traceAccordion).toContainText(/started_at/, {
      timeout: 15000,
    })

    // Neither expander rendered the "log unavailable" fallback.
    await expect(logUnavailable).toHaveCount(0)
  })
})
