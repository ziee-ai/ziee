import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { seedProjectFile, openPreviewDrawer } from './helpers'

// TEST-10 (ITEM-10): dedicated full-page file view + not-found state.
test.describe('File viewer — full page view', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    await loginAsAdmin(page, testInfra.baseURL)
  })

  test('navigates to /files/:id, closes the drawer, back returns', async ({ page, testInfra }) => {
    const fileId = await seedProjectFile(page, testInfra.baseURL, {
      projectName: `Full ${Date.now()}`,
      filename: 'full.txt',
      content: 'full page content marker',
      mime: 'text/plain',
    })
    const drawer = await openPreviewDrawer(page, 'full.txt')
    await drawer.getByTestId('file-viewer-fullpage-btn').click()

    await page.waitForURL(new RegExp(`/files/${fileId}$`))
    await expect(page.getByTestId('file-view-page')).toBeVisible()
    // Filename + body render through the reused FilePanel.
    await expect(page.getByText('full.txt', { exact: false }).first()).toBeVisible()
    // Drawer closed on navigate.
    await expect(page.getByRole('dialog')).toHaveCount(0)

    // Back button returns to the originating page.
    await page.getByTestId('file-view-back-btn').click()
    await expect(page).not.toHaveURL(new RegExp(`/files/${fileId}$`))
  })

  test('shows the not-found state for a bogus id', async ({ page, testInfra }) => {
    // Land somewhere authenticated first, then navigate to a non-existent file.
    await seedProjectFile(page, testInfra.baseURL, {
      projectName: `NF ${Date.now()}`,
      filename: 'nf.txt',
      content: 'x',
      mime: 'text/plain',
    })
    await page.goto(`${testInfra.baseURL}/files/00000000-0000-0000-0000-000000000000`)
    await expect(page.getByTestId('file-view-not-found')).toBeVisible({ timeout: 15000 })
  })
})
