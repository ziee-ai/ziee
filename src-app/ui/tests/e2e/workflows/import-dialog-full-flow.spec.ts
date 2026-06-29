import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import {
  goToWorkflowsPage,
  buildWorkflowBundle,
} from './helpers/workflow-helpers'
import { byTestId } from '../testid'

/**
 * E2E — the Import-Workflow dialog FULL flow (ImportWorkflowDialog.tsx).
 *
 * Audit gap (all-32d1d9ce55b4): `import-dialog-validate.spec.ts` covers only
 * the Validate → success-Alert half. Two real branches had no E2E:
 *   1. the actual IMPORT — dropping a workflow bundle into the Dragger and
 *      clicking "Import" must POST the multipart bundle, create the workflow,
 *      and surface it on the list (handleImport, ImportWorkflowDialog.tsx:71-99);
 *   2. the validation-ERROR Alert branch (ImportWorkflowDialog.tsx:134-162),
 *      which the success-only validate spec never renders.
 *
 * Only the file upload is synthetic; the import + validate HTTP round-trips
 * run for real against the backend (no route mocks).
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

// Missing `steps:` entirely → the server-side validator rejects it, so the
// dialog renders the error ("Validation failed") Alert branch.
const INVALID_WORKFLOW_YAML = `$schema: "/schemas/2026-06-12/workflow-definition.schema.json"
inputs:
  - name: topic
    required: true
`

test.describe('Workflows — Import dialog full flow', () => {
  test('drop a bundle → Import → workflow is created and appears on the list', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await goToWorkflowsPage(page, baseURL)

    await byTestId(page, 'wf-list-import-btn').click()

    const dialog = byTestId(page, 'wf-import-dialog')
    await expect(dialog).toBeVisible()

    // Drop a real tar.gz bundle (the production import format, identical to the
    // one `seedDevWorkflow` posts) into the antd Dragger's hidden file input.
    await dialog.locator('input[type="file"]').setInputFiles({
      name: 'bundle.tar.gz',
      mimeType: 'application/gzip',
      buffer: buildWorkflowBundle(VALID_WORKFLOW_YAML),
    })

    // Click Import and assert the REAL multipart import round-trip 201s.
    const importResp = page.waitForResponse(
      r =>
        r.url().includes('/api/workflows/import') &&
        r.request().method() === 'POST',
      { timeout: 30000 },
    )
    await byTestId(dialog, 'wf-import-submit-btn').click()
    expect((await importResp).status()).toBe(201)

    // The dialog closes and the success toast fires.
    await expect(
      page.locator('[data-sonner-toast][data-type="success"]'),
    ).toContainText('Workflow imported', { timeout: 15000 })
    await expect(dialog).toBeHidden({ timeout: 15000 })

    // The imported workflow surfaces on the list (display_name === slug ===
    // "imported-workflow"; see dev.rs import_workflow_inner).
    await goToWorkflowsPage(page, baseURL)
    await expect(
      page
        .locator('[data-testid^="wf-list-card-"]')
        .filter({ hasText: 'imported-workflow' })
        .first(),
    ).toBeVisible({ timeout: 15000 })
  })

  test('drop an invalid workflow.yaml → Validate → error Alert', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await goToWorkflowsPage(page, baseURL)

    await byTestId(page, 'wf-list-import-btn').click()
    const dialog = byTestId(page, 'wf-import-dialog')
    await expect(dialog).toBeVisible()

    await dialog.locator('input[type="file"]').setInputFiles({
      name: 'workflow.yaml',
      mimeType: 'text/yaml',
      buffer: Buffer.from(INVALID_WORKFLOW_YAML, 'utf8'),
    })

    await byTestId(dialog, 'wf-import-validate-btn').click()

    // The real /validate round-trip returns valid:false → the error-type Alert.
    await expect(byTestId(dialog, 'wf-import-validation-alert')).toContainText(
      'Validation failed',
      { timeout: 30000 },
    )
  })
})
