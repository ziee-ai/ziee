import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import {
  goToWorkflowsSettingsPage,
  openWorkflowCard,
  seedDevWorkflow,
} from './helpers/workflow-helpers'

/**
 * Dry-run preview dialog (`DryRunPreviewDialog`) — E2E coverage gap.
 *
 * The dialog POSTs `/api/workflows/{id}/dry-run` and renders per-step estimated
 * call/token counts + an aggregate "Est. calls" / "Est. tokens" header. Unlike a
 * real run it never calls an LLM, so this needs NO ANTHROPIC_API_KEY — the
 * estimate is computed statically from the workflow definition.
 *
 * Flow: login → seed a 2-step `llm` workflow via the API → open its drawer on
 * /settings/workflows → click "Dry-run preview" → assert the dialog opens with
 * the aggregate statistics and a row per step.
 */

const DRYRUN_WORKFLOW_YAML = `$schema: "/schemas/2026-06-12/workflow-definition.schema.json"
inputs:
  - name: topic
    description: "subject"
    required: true
steps:
  - id: first
    kind: llm
    message: "First step on {{ inputs.topic }}"
    prompt: |
      One sentence about "{{ inputs.topic }}".
  - id: second
    kind: llm
    message: "Second step"
    prompt: |
      One more sentence about "{{ inputs.topic }}".
outputs:
  - name: summary
    from: "{{ second.output }}"
    expose: full
`

test.describe('Workflows - dry-run preview', () => {
  test('opens the dry-run dialog and shows per-step estimates', async ({
    page,
    request,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra

    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)

    await seedDevWorkflow(
      request,
      apiURL,
      adminToken,
      'e2e-dryrun-preview',
      DRYRUN_WORKFLOW_YAML,
    )

    await goToWorkflowsSettingsPage(page, baseURL)
    await openWorkflowCard(page, 'e2e-dryrun-preview')

    // Click the "Dry-run preview" button in the drawer.
    await page
      .getByRole('button', { name: /Dry-run preview/ })
      .first()
      .click()

    // The dialog opens with its title + aggregate statistics.
    const dialog = page.getByRole('dialog', { name: 'Dry-run preview' })
    await expect(dialog).toBeVisible({ timeout: 10000 })
    await expect(dialog.getByText('Est. calls')).toBeVisible({ timeout: 10000 })
    await expect(dialog.getByText('Est. tokens')).toBeVisible()

    // The per-step estimate table lists both steps by id.
    await expect(dialog.getByText('first', { exact: true })).toBeVisible()
    await expect(dialog.getByText('second', { exact: true })).toBeVisible()

    // The estimates-only disclaimer is present.
    await expect(dialog.getByText(/Estimates only/)).toBeVisible()
  })
})
