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
    await page.getByRole('button', { name: /Run$/ }).first().click()
    // The Run dialog (a Modal titled "Run <workflow>") opened — target it by
    // dialog role + name, not getByText(/^Run /), which also matches the
    // drawer's "Run tests" button text behind the modal (strict-mode clash).
    await expect(page.getByRole('dialog', { name: /^Run / })).toBeVisible({
      timeout: 10000,
    })
    const topicField = page.getByLabel('topic')
    if (await topicField.count()) {
      await topicField.first().fill('photosynthesis')
    } else {
      await page.getByPlaceholder(/"topic"/).fill('{ "topic": "photosynthesis" }')
    }
    await page.getByLabel('Model').click()
    await page.getByRole('option', { name: /Claude Haiku 4\.5/ }).first().click()
    await page.getByRole('button', { name: 'Run', exact: true }).last().click()

    // Wait for the run to complete. Both the run-level status Tag and the
    // step's status Tag read "completed", so target the first (the header tag).
    await expect(
      page.getByText('completed', { exact: true }).first(),
    ).toBeVisible({ timeout: 60000 })

    // The Runs section lists the run with the "Workflow page" source badge.
    await expect(page.getByText('Runs', { exact: true })).toBeVisible()
    await expect(
      page.getByText('Workflow page', { exact: true }).first(),
    ).toBeVisible({ timeout: 15000 })

    // Click the run row → the run-progress view shows it.
    await page.getByText('Workflow page', { exact: true }).first().click()
    await expect(page.getByText('Run progress')).toBeVisible({ timeout: 10000 })

    // Delete the run from the Runs list → Popconfirm → confirm. The per-row
    // delete is an icon-only Button whose accessible name is exactly "delete"
    // (the DeleteOutlined icon's aria-label). Match it exactly so we don't grab
    // the drawer's workflow "delete Delete" button (which a bare /delete/i would
    // hit first in DOM order, deleting the workflow instead of the run).
    const deleteBtn = page
      .getByRole('button', { name: 'delete', exact: true })
      .first()
    await deleteBtn.click()
    // Popconfirm: "Delete this run?" with an OK button labeled "Delete".
    await expect(page.getByText(/delete this run\?/i)).toBeVisible()
    await page
      .getByRole('button', { name: 'Delete', exact: true })
      .last()
      .click()

    // The run row is gone from the list.
    await expect(
      page.getByText('Workflow page', { exact: true }),
    ).toHaveCount(0, { timeout: 15000 })
  })
})
