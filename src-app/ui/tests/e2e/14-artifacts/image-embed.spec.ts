import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { seedProjectFile } from '../file/helpers'

// TEST-21 (ITEM-21): dropping (or pasting) an image into the markdown canvas
// uploads it via POST /api/files/upload and inserts an image node that
// serializes to a markdown `![](/api/files/{id}/raw)` link — surviving Save +
// reload. The editor's native handler serves both drop and paste; this spec
// drives the drop path (Chromium honors DragEvent.dataTransfer for synthetic
// dispatch, unlike ClipboardEvent's read-only clipboardData).
//
// A 1x1 transparent PNG (base64).
const PNG_1x1 =
  'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAAC0lEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg=='

test.describe('Artifacts — image paste-embed', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    await loginAsAdmin(page, testInfra.baseURL)
  })

  test('drop an image → upload + insert → persists on reload', async ({
    page,
    testInfra,
  }) => {
    const fileId = await seedProjectFile(page, testInfra.baseURL, {
      projectName: `Image ${Date.now()}`,
      filename: 'illustrated.md',
      content: 'Before the image.\n',
      mime: 'text/markdown',
    })

    await page.goto(`${testInfra.baseURL}/files/${fileId}`)
    await page.getByTestId('canvas-edit-toggle').click()
    const editable = page
      .getByTestId('canvas-edit-body')
      .locator('[contenteditable="true"]')
      .first()
    await expect(editable).toBeVisible()
    await editable.click()

    // Dispatch a synthetic DROP carrying a PNG File — Chromium honors the
    // DragEvent constructor's `dataTransfer` (unlike ClipboardEvent's read-only
    // clipboardData), so the editor's native drop handler uploads it (real POST
    // /api/files/upload) and inserts an <img>.
    await editable.evaluate((el, b64) => {
      const bytes = Uint8Array.from(atob(b64), (c) => c.charCodeAt(0))
      const file = new File([bytes], 'dropped.png', { type: 'image/png' })
      const dt = new DataTransfer()
      dt.items.add(file)
      el.dispatchEvent(
        new DragEvent('drop', { dataTransfer: dt, bubbles: true, cancelable: true }),
      )
    }, PNG_1x1)

    // The uploaded image renders in the editor.
    const img = page.getByTestId('canvas-image')
    await expect(img).toBeVisible({ timeout: 15000 })
    await expect(img).toHaveAttribute('src', /\/api\/files\/[0-9a-f-]+\/raw/)

    // Save appends a version; reload + re-open Edit → the image persists (the
    // saved markdown carried the `![](…)` link, re-parsed into an img node).
    await page.getByTestId('canvas-save').click()
    await expect(page.getByTestId('file-version-bar')).toBeVisible()
    await page.reload()
    await page.getByTestId('canvas-edit-toggle').click()
    await expect(page.getByTestId('canvas-image')).toBeVisible({ timeout: 15000 })
  })
})
