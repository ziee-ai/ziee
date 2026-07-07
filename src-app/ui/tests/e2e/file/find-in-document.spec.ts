import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { seedProjectFile, openPreviewDrawer } from './helpers'

// TEST-6 (ITEM-4, ITEM-5, ITEM-8): find-in-document over a text viewer.
const CONTENT = [
  'alpha beta gamma',
  'delta alpha epsilon',
  'zeta alpha eta',
  'theta iota kappa',
].join('\n')

test.describe('File viewer — find in document', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    await loginAsAdmin(page, testInfra.baseURL)
  })

  test('open (button + Ctrl-F), count, next/prev, close', async ({ page, testInfra }) => {
    await seedProjectFile(page, testInfra.baseURL, {
      projectName: `Find ${Date.now()}`,
      filename: 'find.txt',
      content: CONTENT,
      mime: 'text/plain',
    })
    const drawer = await openPreviewDrawer(page, 'find.txt')
    // Body renders the raw code view.
    await drawer.getByTestId('raw-code-view').waitFor({ state: 'visible' })

    // Open the find bar via the header button.
    await drawer.getByTestId('file-viewer-find-btn').click()
    const input = drawer.getByTestId('file-find-input')
    await expect(input).toBeVisible()

    // "alpha" occurs 3× → count shows 1 / 3.
    await input.fill('alpha')
    const count = drawer.getByTestId('file-find-count')
    await expect(count).toHaveText('1 / 3')

    // Next advances the active index (wraps at the end).
    await drawer.getByTestId('file-find-next-btn').click()
    await expect(count).toHaveText('2 / 3')
    await drawer.getByTestId('file-find-next-btn').click()
    await expect(count).toHaveText('3 / 3')
    await drawer.getByTestId('file-find-next-btn').click()
    await expect(count).toHaveText('1 / 3')

    // Prev wraps backward.
    await drawer.getByTestId('file-find-prev-btn').click()
    await expect(count).toHaveText('3 / 3')

    // A no-match query reports "No results".
    await input.fill('zzzznotfound')
    await expect(count).toHaveText('No results')

    // Escape closes the bar.
    await input.fill('alpha')
    await input.press('Escape')
    await expect(drawer.getByTestId('file-find-bar')).toHaveCount(0)

    // Ctrl-F re-opens it (region-scoped shortcut).
    await drawer.getByTestId('raw-code-view').click()
    await page.keyboard.press('Control+f')
    await expect(drawer.getByTestId('file-find-input')).toBeVisible()
  })
})
