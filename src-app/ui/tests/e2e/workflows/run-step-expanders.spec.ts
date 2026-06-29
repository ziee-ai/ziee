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
 * Per-step log expander buttons in the Workflow Run Progress View
 * (audit gap all-8e1c443fb4de).
 *
 * `WorkflowRunProgressView.tsx:283-315` renders, for every completed/failed
 * step, a row of `StepLogExpanderLocal` link buttons — "Show prompt",
 * "Show raw output", ("Show stderr" for sandbox steps), "Show trace" — each
 * of which lazily `ApiClient.Workflow.readLog`s its kind and toggles the body
 * open. `run-a-workflow.spec.ts` clicks "Show prompt"/"Show trace" but ONLY on
 * the real-LLM tier, and never exercises "Show raw output" nor the
 * `read_log` → 404 → "Log not available" fallback branch.
 *
 * This spec covers those expanders DETERMINISTICALLY (no LLM, no rootfs) using
 * the runner's dev-only per-step `mock:` short-circuit: a seeded dev workflow's
 * `llm` step with a baked `mock` value completes WITHOUT dispatching a model
 * (`runner.rs` `run_mock_step`). A mocked step:
 *   - writes its `trace.json` (the `StepResult::Completed` arm calls
 *     `write_trace` unconditionally) → "Show trace" loads real content; and
 *   - captures NO prompt/raw_output (the mock skips dispatch) → "Show prompt" /
 *     "Show raw output" hit `read_log`'s `WORKFLOW_LOG_MISSING` 404 → the
 *     expander renders its "Log not available" fallback.
 * So one deterministic run exercises BOTH the success-render and the
 * 404-fallback render paths of `StepLogExpanderLocal`. Nothing is mocked at the
 * HTTP layer — the `mock:` is a first-class workflow-runner feature.
 *
 * A second, real-LLM-gated test asserts "Show raw output" surfaces the real
 * captured model output (the one expander branch the mock can't fill with
 * content), mirroring `run-a-workflow.spec.ts`'s gating.
 */

// A single `llm` step with a baked dev-only `mock` → completes deterministically
// (no model call). `description` gives the step a stable visible label.
const MOCKED_WORKFLOW_YAML = `$schema: "/schemas/2026-06-12/workflow-definition.schema.json"
inputs: []
steps:
  - id: summarize
    kind: llm
    description: "Summarize the canned topic"
    prompt: |
      Say one short sentence.
    mock: "MOCKED_STEP_OUTPUT_BEACON"
outputs:
  - name: summary
    from: "{{ summarize.output }}"
    expose: full
`

test.describe('Workflows - run progress per-step log expanders', () => {
  test('mocked step: Show raw output/prompt fall back to "Log not available", Show trace loads', async ({
    page,
    request,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra

    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)

    // A model just so the standalone Run dialog's picker is non-empty. The
    // single llm step is `mock`-short-circuited, so this model is never
    // invoked — a dummy provider key keeps the test off the real-LLM tier.
    const providerId = await createProviderViaAPI(
      apiURL,
      adminToken,
      'OpenAI',
      'openai',
    )
    await assignProviderToAdministratorsGroup(apiURL, adminToken, providerId)
    await createModelViaAPI(
      apiURL,
      adminToken,
      providerId,
      undefined,
      undefined,
      'openai',
    )

    await seedDevWorkflow(
      request,
      apiURL,
      adminToken,
      'e2e-expander-mock',
      MOCKED_WORKFLOW_YAML,
    )

    await goToWorkflowsSettingsPage(page, baseURL)
    await openWorkflowCard(page, 'e2e-expander-mock')

    // Open the Run dialog from the drawer (drawer button name is the icon
    // aria-label + "Run"; match the trailing word).
    await byTestId(page, 'wf-detail-run-btn').click()
    await expect(byTestId(page, 'wf-run-dialog')).toBeVisible({
      timeout: 10000,
    })

    // Pick the registered model (required for a standalone run with an llm step).
    await byTestId(page, 'wf-run-model-select').click()
    await page
      .locator('[data-testid^="wf-run-model-select-opt-"]')
      .first()
      .click()

    // Kick the run (the dialog's own OK "Run" button).
    await byTestId(page, 'wf-run-submit-btn').click()

    // Run-progress view appears and the (mocked) run completes.
    await expect(byTestId(page, 'wf-progress-status-tag')).toBeVisible({
      timeout: 15000,
    })
    await expect(byTestId(page, 'wf-progress-status-tag')).toContainText(
      'completed',
      { timeout: 30000 },
    )

    // The completed step exposes its per-step log expander buttons.
    const rawOutputBtn = page.locator(
      '[data-testid^="wf-step-log-btn-"][data-testid$="-raw_output"]',
    )
    const promptBtn = page.locator(
      '[data-testid^="wf-step-log-btn-"][data-testid$="-prompt"]',
    )
    const traceBtn = page.locator(
      '[data-testid^="wf-step-log-btn-"][data-testid$="-trace"]',
    )
    const logUnavailable = page.locator('[data-testid="wf-step-log-empty"]')
    const traceAccordion = page
      .locator('[data-testid^="wf-step-log-accordion-"][data-testid$="-trace"]')
      .first()
    await expect(rawOutputBtn.first()).toBeVisible({ timeout: 10000 })
    await expect(promptBtn.first()).toBeVisible()
    await expect(traceBtn.first()).toBeVisible()

    // "Show raw output": the mock captured no dispatch logs, so read_log 404s
    // and the expander renders its "Log not available" fallback (the 404 branch
    // of StepLogExpander.fetchLog — previously untested). Toggling open
    // triggers the real fetch.
    await rawOutputBtn.first().click()
    await expect(logUnavailable.first()).toBeVisible({
      timeout: 15000,
    })

    // "Show prompt": same fallback (a mock writes no prompt log).
    await promptBtn.first().click()
    await expect(logUnavailable).toHaveCount(2, {
      timeout: 15000,
    })

    // "Show trace": a completed step DID write trace.json, so this expander
    // loads real content — the serialized StepTrace JSON (stable `attempts`
    // field) — exercising the success-render path of the same component.
    await traceBtn.first().click()
    await expect(traceAccordion).toContainText(/attempts/i, {
      timeout: 15000,
    })
  })

  test('real-LLM: Show raw output surfaces the captured model output', async ({
    page,
    request,
    testInfra,
  }) => {
    const ANTHROPIC_KEY = process.env.ANTHROPIC_API_KEY ?? ''
    test.skip(
      ANTHROPIC_KEY.length === 0,
      'ANTHROPIC_API_KEY not set — real-LLM raw-output leg skipped',
    )

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

    // A real one-step llm workflow that must echo a unique beacon, so the
    // captured raw_output is assertable.
    const beacon = `RAWOUT_BEACON_${Date.now().toString(36)}`
    const yaml = `$schema: "/schemas/2026-06-12/workflow-definition.schema.json"
inputs: []
steps:
  - id: echo
    kind: llm
    description: "Echo the beacon"
    prompt: |
      Reply with EXACTLY this token and nothing else: ${beacon}
outputs:
  - name: out
    from: "{{ echo.output }}"
    expose: full
`
    await seedDevWorkflow(request, apiURL, adminToken, 'e2e-expander-real', yaml)

    await goToWorkflowsSettingsPage(page, baseURL)
    await openWorkflowCard(page, 'e2e-expander-real')

    await byTestId(page, 'wf-detail-run-btn').click()
    await expect(byTestId(page, 'wf-run-dialog')).toBeVisible({
      timeout: 10000,
    })

    await byTestId(page, 'wf-run-model-select').click()
    await page
      .locator('[data-testid^="wf-run-model-select-opt-"]')
      .first()
      .click()

    // Turn on "Capture debug logs" so prompt/raw_output are persisted.
    const captureToggle = byTestId(page, 'wf-run-capture-logs-switch')
    await captureToggle.click()
    await expect(captureToggle).toBeChecked()

    await byTestId(page, 'wf-run-submit-btn').click()

    await expect(byTestId(page, 'wf-progress-status-tag')).toBeVisible({
      timeout: 15000,
    })
    await expect(byTestId(page, 'wf-progress-status-tag')).toContainText(
      'completed',
      { timeout: 60000 },
    )

    // The captured raw model output is revealed by the "Show raw output"
    // expander and contains the beacon the model was told to echo.
    await page
      .locator('[data-testid^="wf-step-log-btn-"][data-testid$="-raw_output"]')
      .first()
      .click()
    await expect(
      page
        .locator(
          '[data-testid^="wf-step-log-accordion-"][data-testid$="-raw_output"]',
        )
        .first(),
    ).toContainText(new RegExp(beacon), {
      timeout: 15000,
    })
  })
})
