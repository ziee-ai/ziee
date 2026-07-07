import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { seedProjectFile, openPreviewDrawer } from './helpers'

// TEST-9 (ITEM-9): Open-in-new-tab opens the token-authenticated raw file.
test.describe('File viewer — open in new tab', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    await loginAsAdmin(page, testInfra.baseURL)
  })

  test('opens the download-with-token URL in a new tab', async ({ page, testInfra }) => {
    // The token endpoint responds with Content-Disposition: attachment, so the new
    // tab DOWNLOADS rather than navigating — `popup.url()` never settles on the
    // token URL. Capture the exact argument passed to window.open instead, which is
    // what the feature actually does.
    await page.addInitScript(() => {
      ;(window as unknown as { __opened: string[] }).__opened = []
      const orig = window.open.bind(window)
      window.open = ((u?: string | URL, ...rest: unknown[]) => {
        if (typeof u === 'string') (window as unknown as { __opened: string[] }).__opened.push(u)
        // Don't actually open (avoids a real download in CI); mimic noopener null.
        void orig
        return null
      }) as typeof window.open
    })
    const fileId = await seedProjectFile(page, testInfra.baseURL, {
      projectName: `Tab ${Date.now()}`,
      filename: 'tab.txt',
      content: 'open me in a tab',
      mime: 'text/plain',
    })
    const drawer = await openPreviewDrawer(page, 'tab.txt')

    await drawer.getByTestId('file-viewer-open-tab-btn').click()
    await expect
      .poll(() => page.evaluate(() => (window as unknown as { __opened: string[] }).__opened[0] ?? ''))
      .toContain(`/api/files/${fileId}/download-with-token?token=`)
  })
})
