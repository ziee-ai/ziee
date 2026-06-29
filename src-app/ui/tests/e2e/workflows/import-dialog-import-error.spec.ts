import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { goToWorkflowsPage } from './helpers/workflow-helpers'
import { byTestId } from '../testid'

/**
 * E2E — the Import-Workflow dialog IMPORT-FAILURE branch (ImportWorkflowDialog.tsx).
 *
 * Audit gap (r2-b0b96a6e993e): `import-dialog-validate.spec.ts` and
 * `import-dialog-full-flow.spec.ts` together cover Validate→success,
 * Validate→error, and the happy Import (create→list→toast). But the Import
 * button is NOT gated on a prior Validate, so a user can click Import on a
 * bundle the server rejects at extract/import time — `handleImport`'s catch
 * (ImportWorkflowDialog.tsx:85-87) then surfaces an error message and, via
 * `setSubmitting(false)`, KEEPS THE DIALOG OPEN (it never reaches `onClose()`).
 * That failure branch — distinct from the `/validate` error Alert — had no E2E.
 *
 * This drops a malformed (non-tar) bundle straight into the Dragger and clicks
 * Import: the real `POST /api/workflows/import` round-trip rejects it (4xx),
 * an error message surfaces, and the dialog stays open so the user can retry.
 * Nothing is mocked but the synthetic file bytes.
 */

test.describe('Workflows — Import dialog import-failure branch', () => {
  test('drop a malformed bundle → Import → error surfaces and the dialog stays open', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await goToWorkflowsPage(page, baseURL)

    await byTestId(page, 'wf-list-import-btn').click()

    const dialog = byTestId(page, 'wf-import-dialog')
    await expect(dialog).toBeVisible()

    // Not a valid tar(.gz) bundle — the server's extractor cannot read a
    // workflow.yaml out of it, so the import endpoint rejects it. The Dragger
    // accepts any file (beforeUpload returns false), so this reaches Import.
    await dialog.locator('input[type="file"]').setInputFiles({
      name: 'broken-bundle.tar.gz',
      mimeType: 'application/gzip',
      buffer: Buffer.from('this is not a valid workflow bundle archive'),
    })

    // The REAL import round-trip must reject (4xx) — no workflow is created.
    const importResp = page.waitForResponse(
      r =>
        r.url().includes('/api/workflows/import') &&
        r.request().method() === 'POST',
      { timeout: 30000 },
    )
    await byTestId(dialog, 'wf-import-submit-btn').click()
    expect((await importResp).status()).toBeGreaterThanOrEqual(400)

    // The catch branch surfaces an error toast (the server's message or the
    // "Import failed" fallback) — assert an error notice renders.
    await expect(
      page.locator('[data-sonner-toast][data-type="error"]').first(),
    ).toBeVisible({ timeout: 15000 })

    // The defining behavior of the failure branch: the dialog STAYS OPEN (the
    // success path would have closed it), so the user can correct + retry.
    await expect(dialog).toBeVisible()
    // The success toast must NOT have fired.
    await expect(
      page.locator('[data-sonner-toast][data-type="success"]'),
    ).toHaveCount(0)
  })
})
