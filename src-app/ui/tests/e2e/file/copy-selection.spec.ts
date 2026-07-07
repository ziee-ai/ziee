import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { seedProjectFile, openPreviewDrawer } from './helpers'

// TEST-8 (ITEM-7, ITEM-8): copy-selection copies the selected text; empty
// selection warns and leaves the clipboard untouched.
const CONTENT = 'copy-me-marker and some other words to select'

test.describe('File viewer — copy selection', () => {
  test.beforeEach(async ({ page, context, testInfra }) => {
    await context.grantPermissions(['clipboard-read', 'clipboard-write'])
    await loginAsAdmin(page, testInfra.baseURL)
  })

  test('copies the current selection; warns on empty selection', async ({ page, testInfra }) => {
    await seedProjectFile(page, testInfra.baseURL, {
      projectName: `Copy ${Date.now()}`,
      filename: 'copy.txt',
      content: CONTENT,
      mime: 'text/plain',
    })
    const drawer = await openPreviewDrawer(page, 'copy.txt')
    const raw = drawer.getByTestId('raw-code-view')
    await raw.waitFor({ state: 'visible' })

    // Seed the clipboard with a sentinel so we can prove empty-selection leaves it.
    await page.evaluate(() => navigator.clipboard.writeText('__sentinel__'))

    // Empty selection → warning, clipboard unchanged.
    await page.evaluate(() => window.getSelection()?.removeAllRanges())
    await drawer.getByTestId('file-viewer-copy-selection-btn').click()
    expect(await page.evaluate(() => navigator.clipboard.readText())).toBe('__sentinel__')

    // Select the marker word inside the body, then copy the selection.
    await raw.evaluate(el => {
      const walker = document.createTreeWalker(el, NodeFilter.SHOW_TEXT)
      for (let n = walker.nextNode(); n; n = walker.nextNode()) {
        const idx = (n.textContent ?? '').indexOf('copy-me-marker')
        if (idx >= 0) {
          const range = document.createRange()
          range.setStart(n, idx)
          range.setEnd(n, idx + 'copy-me-marker'.length)
          const sel = window.getSelection()!
          sel.removeAllRanges()
          sel.addRange(range)
          return
        }
      }
    })
    await drawer.getByTestId('file-viewer-copy-selection-btn').click()
    await expect
      .poll(() => page.evaluate(() => navigator.clipboard.readText()))
      .toBe('copy-me-marker')
  })
})
