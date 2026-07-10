import { closeSync, ftruncateSync, openSync, rmSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { goToNewChatPage } from './helpers/chat-helpers'
import { byTestId } from '../testid'

/**
 * E2E — client-side per-file size cap in the chat composer. Attaching a file
 * larger than the shared `MAX_FILE_UPLOAD_BYTES` (128 MiB) must surface a
 * "128MB" too-large toast and fire NO upload request (the client pre-rejects
 * before contacting the server).
 *
 * The oversize file is created SPARSE via ftruncate — the browser reads
 * `File.size` from filesystem metadata, so no 128 MiB is written or transferred.
 */

test.describe('Chat — file upload size limit', () => {
  test('an over-cap file is rejected client-side with the 128MB message', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)

    // Track whether any upload request is attempted (there must be none).
    let uploadAttempted = false
    await page.route(/\/api\/files\/upload$/, async (route, req) => {
      if (req.method() === 'POST') uploadAttempted = true
      return route.fallback()
    })

    await goToNewChatPage(page, baseURL)

    // Create a sparse file just over the 128 MiB cap.
    const oversizePath = join(tmpdir(), `ziee-oversize-${Date.now()}.bin`)
    const fd = openSync(oversizePath, 'w')
    ftruncateSync(fd, 128 * 1024 * 1024 + 1)
    closeSync(fd)

    try {
      await byTestId(page, 'chat-input-add-btn').click()
      const [fileChooser] = await Promise.all([
        page.waitForEvent('filechooser'),
        page.getByText('Attach files or photos').click(),
      ])
      await fileChooser.setFiles(oversizePath)

      // The too-large toast names the 128MB limit.
      await expect(
        page.getByText(/Maximum size is 128MB/i).first(),
      ).toBeVisible({ timeout: 10000 })

      // No file card was created and no upload request fired.
      await expect(
        page.locator('[data-testid="file-card-uploading"]'),
      ).toHaveCount(0)
      expect(uploadAttempted).toBe(false)
    } finally {
      rmSync(oversizePath, { force: true })
    }
  })
})
