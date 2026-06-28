import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { goToNewChatPage } from './helpers/chat-helpers'

/**
 * E2E — file upload ERROR + CANCEL through the chat composer (File.store
 * uploadFiles catch → status:'error'; FilePreviewList renders the error FileCard
 * with a remove/cancel control). The upload endpoint is forced to 500 (external
 * boundary) so the real error-handling path runs; the error card + its removal
 * are the behavior under test.
 */

test.describe('Chat — file upload error + cancel', () => {
  test('a failed upload shows the error card and can be removed', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)

    // Force the file upload to fail.
    await page.route(/\/api\/files\/upload$/, async (route, req) => {
      if (req.method() === 'POST') {
        return route.fulfill({
          status: 500,
          contentType: 'application/json',
          body: JSON.stringify({ error: { message: 'upload boom' } }),
        })
      }
      return route.fallback()
    })

    await goToNewChatPage(page, baseURL)

    // Attach a file via the composer "+" menu → native file chooser.
    await page.getByRole('button', { name: 'Add attachment' }).click()
    const [fileChooser] = await Promise.all([
      page.waitForEvent('filechooser'),
      page.getByText('Attach files or photos').click(),
    ])
    await fileChooser.setFiles({
      name: 'broken-upload.txt',
      mimeType: 'text/plain',
      buffer: Buffer.from('content that will fail to upload'),
    })

    // The upload fails → the error FileCard renders (ERROR badge).
    const errorCard = page
      .locator('[data-testid="file-card-uploading"]')
      .filter({ hasText: 'ERROR' })
    await expect(errorCard.first()).toBeVisible({ timeout: 30000 })

    // Cancel/remove the errored upload → the card disappears.
    await errorCard.first().getByRole('button').last().click()
    await expect(
      page.locator('[data-testid="file-card-uploading"]').filter({ hasText: 'ERROR' }),
    ).toHaveCount(0, { timeout: 10000 })
  })
})
