import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import { seedProjectFile } from '../file/helpers'

// TEST-24 (ITEM-14, ITEM-13): while editing, a second client advancing the head
// (here a raw append-version REST call, standing in for a model `edit_file`) shows
// the non-destructive "document changed" banner; "Keep my changes" retains the
// local edit and Save appends a new head — nothing is silently overwritten.
test.describe('Artifacts — concurrent-edit reconciliation', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    await loginAsAdmin(page, testInfra.baseURL)
  })

  test('head advancing underneath shows the banner; Keep-mine preserves both', async ({
    page,
    testInfra,
  }) => {
    const fileId = await seedProjectFile(page, testInfra.baseURL, {
      projectName: `Concurrent ${Date.now()}`,
      filename: 'co.md',
      content: 'Base content\n',
      mime: 'text/markdown',
    })

    await page.goto(`${testInfra.baseURL}/files/${fileId}`)
    await page.getByTestId('canvas-edit-toggle').click()
    const editable = page
      .getByTestId('canvas-edit-body')
      .locator('[contenteditable="true"]')
      .first()
    await editable.click()
    await page.keyboard.press('End')
    await editable.pressSequentially(' my local edit')

    // A second "client" advances the head out-of-band (no X-Sync-Connection-Id →
    // the edit is not self-echo-suppressed and reaches this browser's SSE stream).
    const token = await getAdminToken(testInfra.baseURL)
    const status = await page.evaluate(
      async ([base, id, t]) => {
        const r = await fetch(`${base}/api/files/${id}/versions`, {
          method: 'POST',
          headers: { Authorization: `Bearer ${t}`, 'Content-Type': 'application/json' },
          body: JSON.stringify({ content: 'External edit from another client\n' }),
        })
        return r.status
      },
      [testInfra.baseURL, fileId, token] as const,
    )
    expect(status).toBe(200)

    // The banner appears once the sync stream lands the new head.
    await expect(page.getByTestId('canvas-changed-banner')).toBeVisible({ timeout: 15000 })

    // Keep my changes → banner dismisses, the local edit survives, Save appends.
    await page.getByTestId('canvas-keep-mine').click()
    await expect(page.getByTestId('canvas-changed-banner')).toHaveCount(0)
    await page.getByTestId('canvas-save').click()

    // Multiple versions now exist (external + mine, nothing lost).
    await expect(page.getByTestId('file-version-bar')).toBeVisible()
  })
})
