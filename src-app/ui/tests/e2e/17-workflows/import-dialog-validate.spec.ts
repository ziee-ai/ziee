import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { goToWorkflowsPage } from './helpers/workflow-helpers'
import { byTestId } from '../testid'

/**
 * E2E — the Import-Workflow dialog (ImportWorkflowDialog.tsx).
 *
 * Audit gap: `list-page-renders.spec.ts` only asserts the gated "Import"
 * button is VISIBLE; the dialog it opens (drop a workflow.yaml → "Validate"
 * → server-side /api/workflows/validate → inline result Alert) had no E2E.
 * This opens the dialog, drops a valid workflow.yaml, runs Validate, and
 * asserts the success Alert — exercising the real validate round-trip.
 * Only the file upload is synthetic; the validation HTTP path runs for real.
 */

const VALID_WORKFLOW_YAML = `$schema: "/schemas/2026-06-12/workflow-definition.schema.json"
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

test.describe('Workflows — Import dialog validate', () => {
  test('drop a workflow.yaml → Validate → success Alert', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await goToWorkflowsPage(page, baseURL)

    await byTestId(page, 'wf-list-import-btn').click()

    const dialog = byTestId(page, 'wf-import-dialog')
    await expect(dialog).toBeVisible()

    // Drop a workflow.yaml into the antd Dragger's hidden <input type=file>.
    await dialog.locator('input[type="file"]').setInputFiles({
      name: 'workflow.yaml',
      mimeType: 'text/yaml',
      buffer: Buffer.from(VALID_WORKFLOW_YAML, 'utf8'),
    })

    await byTestId(dialog, 'wf-import-validate-btn').click()

    // The /validate round-trip renders the success Alert ("Valid workflow — N steps...").
    await expect(byTestId(dialog, 'wf-import-validation-alert')).toContainText(
      /Valid workflow/,
      { timeout: 30000 },
    )
  })
})
