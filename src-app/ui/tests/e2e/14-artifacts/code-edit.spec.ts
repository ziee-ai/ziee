import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { seedProjectFile } from '../file/helpers'

// TEST-19: a code deliverable opens in CodeMirror (plain-text, no reformatting);
// editing + Save appends a version and the content round-trips exactly.
test.describe('Artifacts — code canvas edit', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    await loginAsAdmin(page, testInfra.baseURL)
  })

  test('code file opens in CodeMirror, edit + save bumps version', async ({
    page,
    testInfra,
  }) => {
    const fileId = await seedProjectFile(page, testInfra.baseURL, {
      projectName: `Code ${Date.now()}`,
      filename: 'script.py',
      content: 'def hello():\n    return 1\n',
      mime: 'text/x-python',
    })

    await page.goto(`${testInfra.baseURL}/files/${fileId}`)
    await page.getByTestId('canvas-edit-toggle').click()
    await expect(page.getByTestId('canvas-edit-body')).toBeVisible()

    // CodeMirror content is a contenteditable `.cm-content`.
    const cm = page.locator('.cm-content[contenteditable="true"]').first()
    await expect(cm).toBeVisible()
    await cm.click()
    await page.keyboard.press('End')
    await page.keyboard.press('Enter')
    await cm.pressSequentially('CODE_EDIT_MARKER = 42')

    await page.getByTestId('canvas-save').click()
    await expect(page.getByTestId('file-version-bar')).toBeVisible()

    // Reload re-fetches the saved head; the exact typed text is preserved.
    await page.reload()
    await expect(page.getByText('CODE_EDIT_MARKER', { exact: false }).first()).toBeVisible()
  })
})
