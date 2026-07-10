import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import { seedProjectFile } from '../file/helpers'

async function headText(page: import('@playwright/test').Page, baseURL: string, fileId: string) {
  const token = await getAdminToken(baseURL)
  return page.evaluate(
    async ([base, id, t]) => {
      const r = await fetch(`${base}/api/files/${id}/text`, {
        headers: { Authorization: `Bearer ${t}` },
      })
      return r.ok ? await r.text() : ''
    },
    [baseURL, fileId, token] as const,
  )
}

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
    // The typed text must actually land in the editor before Save.
    await expect(cm).toContainText('CODE_EDIT_MARKER')

    // Save must become enabled (a real edit set the dirty flag).
    await expect(page.getByTestId('canvas-save')).toBeEnabled()
    await page.getByTestId('canvas-save').click()
    await expect(page.getByTestId('file-version-bar')).toBeVisible()

    // Authoritative persistence: the saved head text carries the exact edit
    // (polled to absorb the async Save→append round-trip). Asserting the server
    // head avoids the read-only viewer's syntax-highlight tokenization.
    await expect
      .poll(() => headText(page, testInfra.baseURL, fileId), { timeout: 15000 })
      .toContain('CODE_EDIT_MARKER')
  })
})
