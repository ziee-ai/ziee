import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import {
  goToWorkflowsSettingsPage,
  openWorkflowCard,
  seedDevWorkflow,
} from './helpers/workflow-helpers'
import { byTestId } from '../testid'

/**
 * E2E — WorkflowRunDialog raw-JSON input mode validation (WorkflowRunDialog.tsx
 * :80-92, 142-155). For an UNSTRUCTURED workflow (no declared inputs) the dialog
 * renders a JSON TextArea; handleRun JSON.parse()s it and on failure sets
 * jsonError, which renders an inline error Alert and aborts the run. Untested.
 */

// No `inputs:` → inputs.length === 0 → the JSON TextArea branch.
const UNSTRUCTURED_YAML = `$schema: "/schemas/2026-06-12/workflow-definition.schema.json"
steps:
  - id: s1
    kind: llm
    message: "Step 1"
    prompt: |
      Do something simple.
outputs:
  - name: out
    from: "{{ s1.output }}"
    expose: full
`

test.describe('Workflows - run dialog JSON validation', () => {
  test('invalid JSON in the raw-input TextArea shows an error and blocks the run', async ({
    page,
    request,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    await seedDevWorkflow(request, apiURL, token, 'e2e-json-validate', UNSTRUCTURED_YAML)

    await goToWorkflowsSettingsPage(page, baseURL)
    await openWorkflowCard(page, 'e2e-json-validate')

    // Open the Run dialog.
    await byTestId(page, 'wf-detail-run-btn').click()
    const dialog = byTestId(page, 'wf-run-dialog')
    await expect(dialog).toBeVisible({ timeout: 10000 })

    // Type malformed JSON, then click the modal's Run (OK) button.
    await byTestId(page, 'wf-run-json-textarea').fill('{ not: valid json,, }')
    await byTestId(page, 'wf-run-submit-btn').click()

    // The validation Alert renders and the dialog stays open (run aborted).
    await expect(byTestId(page, 'wf-run-json-error-alert')).toContainText(
      'Inputs must be valid JSON',
      { timeout: 10000 },
    )
    await expect(dialog).toBeVisible()
  })
})
