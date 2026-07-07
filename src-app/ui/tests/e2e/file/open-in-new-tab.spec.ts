import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { seedProjectFile, openPreviewDrawer } from './helpers'

// TEST-9 (ITEM-9): Open-in-new-tab opens the token-authenticated raw file.
test.describe('File viewer — open in new tab', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    await loginAsAdmin(page, testInfra.baseURL)
  })

  test('opens a popup to the download-with-token URL', async ({ page, context, testInfra }) => {
    const fileId = await seedProjectFile(page, testInfra.baseURL, {
      projectName: `Tab ${Date.now()}`,
      filename: 'tab.txt',
      content: 'open me in a tab',
      mime: 'text/plain',
    })
    const drawer = await openPreviewDrawer(page, 'tab.txt')

    const [popup] = await Promise.all([
      context.waitForEvent('page'),
      drawer.getByTestId('file-viewer-open-tab-btn').click(),
    ])
    await popup.waitForLoadState('domcontentloaded').catch(() => undefined)
    expect(popup.url()).toContain(`/api/files/${fileId}/download-with-token`)
    expect(popup.url()).toContain('token=')
  })
})
