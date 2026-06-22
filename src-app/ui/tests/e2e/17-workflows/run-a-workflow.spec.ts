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
 * `11-projects/message-uses-project-context.spec.ts`. The Rust-side coverage of
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
    await page.getByRole('button', { name: /Run$/ }).first().click()
    // Assert the Run dialog (a Modal titled "Run <workflow>") opened — target
    // it by dialog role + name, not getByText(/^Run /): the latter also matches
    // the drawer's "Run tests" button text behind the modal (strict-mode clash).
    await expect(page.getByRole('dialog', { name: /^Run / })).toBeVisible({
      timeout: 10000,
    })

    // Provide the required `topic` input. With structured inputs the field is
    // labeled by the input name; fall back to the free-form JSON editor.
    const topicField = page.getByLabel('topic')
    if (await topicField.count()) {
      await topicField.first().fill('quantum entanglement')
    } else {
      await page
        .getByPlaceholder(/"topic"/)
        .fill('{ "topic": "quantum entanglement" }')
    }

    // Pick a model in the standalone picker.
    const modelSelect = page.getByLabel('Model')
    await modelSelect.click()
    await page
      .getByRole('option', { name: /Claude Haiku 4\.5/ })
      .first()
      .click()

    // Turn on "Capture debug logs".
    const captureToggle = page.getByRole('switch').first()
    await captureToggle.click()
    await expect(captureToggle).toBeChecked()

    // Run.
    await page
      .getByRole('button', { name: 'Run', exact: true })
      .last()
      .click()

    // The run-progress view appears and streams to completion. The status tag
    // transitions to "completed"; allow a generous budget for the real call.
    await expect(page.getByText('Run progress')).toBeVisible({ timeout: 15000 })
    // Both the run-level status Tag and the (single) step's status Tag read
    // "completed", so target the first (the run-level header tag).
    await expect(
      page.getByText('completed', { exact: true }).first(),
    ).toBeVisible({ timeout: 60000 })

    // A7: a completed LLM step surfaces its per-step log expanders — "Show
    // prompt" (an llm step has a prompt) and "Show trace" (trace.json is
    // written on completion). This guards the durable-log viewers against an
    // antd render-trap and confirms the trace affordance shows for a completed
    // step (it is correctly hidden for failed steps).
    await expect(page.getByText('Show prompt').first()).toBeVisible({
      timeout: 10000,
    })
    await expect(page.getByText('Show trace').first()).toBeVisible()
  })
})
