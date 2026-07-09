import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { seedProjectFile } from '../file/helpers'

// TEST-17 / TEST-18 / TEST-22 / TEST-30: the core canvas round-trip on the real
// built UI (full-page file view /files/:id renders FilePanel with the header, so
// the Edit toggle + export menu are present — the drawer hides the header).
//
// Flow: open a markdown deliverable → Edit loads the Plate WYSIWYG + toolbar →
// apply a heading via the toolbar + type → Save appends a version (FileVersionBar
// appears) → reload re-fetches the saved head → Export-as-markdown downloads a
// file whose bytes reflect the toolbar formatting (end-to-end round-trip).
test.describe('Artifacts — canvas WYSIWYG edit + save + export', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    await loginAsAdmin(page, testInfra.baseURL)
  })

  test('edit via toolbar, save bumps version, reload persists, export reflects it', async ({
    page,
    testInfra,
  }) => {
    const fileId = await seedProjectFile(page, testInfra.baseURL, {
      projectName: `Canvas ${Date.now()}`,
      filename: 'deliverable.md',
      content: 'Initial deliverable body\n',
      mime: 'text/markdown',
    })

    await page.goto(`${testInfra.baseURL}/files/${fileId}`)
    await expect(page.getByTestId('file-view-page')).toBeVisible()

    // Enter edit mode — the WYSIWYG editor + formatting toolbar render.
    await page.getByTestId('canvas-edit-toggle').click()
    const editBody = page.getByTestId('canvas-edit-body')
    await expect(editBody).toBeVisible()
    await expect(page.getByTestId('canvas-markdown-toolbar')).toBeVisible()

    const editable = editBody.locator('[contenteditable="true"]').first()
    await expect(editable).toBeVisible()

    // Type a new line of content, then promote the current block to a heading
    // via the real toolbar button (block transform on the caret's block).
    await editable.click()
    await page.keyboard.press('End')
    await page.keyboard.press('Enter')
    await editable.pressSequentially('Canvas H2 marker')
    await page.getByTestId('canvas-toolbar-h2').click()

    // Save appends a new version.
    await page.getByTestId('canvas-save').click()

    // A second version now exists → the version bar appears with the head tag.
    await expect(page.getByTestId('file-version-bar')).toBeVisible()
    await expect(page.getByTestId('file-version-current-tag')).toBeVisible()

    // Reload re-fetches the saved head from the server — the edit persisted.
    await page.reload()
    await expect(page.getByTestId('file-view-page')).toBeVisible()
    await expect(page.getByText('Canvas H2 marker', { exact: false }).first()).toBeVisible()

    // Export-as-markdown downloads a real file whose bytes carry the heading
    // syntax the toolbar produced (WYSIWYG → markdown round-trip end-to-end).
    await page.getByTestId('file-export-menu').click()
    const [download] = await Promise.all([
      page.waitForEvent('download'),
      page.getByTestId('file-export-md').click(),
    ])
    const path = await download.path()
    const fs = await import('node:fs/promises')
    const exported = await fs.readFile(path!, 'utf-8')
    expect(exported).toContain('Canvas H2 marker')
    expect(exported).toContain('## Canvas H2 marker')
  })
})
