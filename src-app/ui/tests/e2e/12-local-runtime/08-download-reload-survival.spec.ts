import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { gotoRuntimeSettings } from './helpers/local-runtime-helpers'

/**
 * In-progress engine downloads survive a page reload: the
 * RuntimeDownloadProgress store runs loadActive() as its `activeByKey` init on
 * every module init, re-pulling every in-flight task (and re-opening its SSE
 * subscription) — so a reload mid-download rehydrates the progress instead of
 * losing it. We mock the active-downloads endpoint with a non-terminal task and
 * assert it is re-fetched on the post-reload mount.
 */
test.describe('Local Runtime — in-progress download reload survival', () => {
  test('active-downloads is re-fetched after a reload', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)

    let listCalls = 0
    await page.route(
      /\/api\/local-runtime\/versions\/downloads$/,
      async route => {
        listCalls += 1
        await route.fulfill({
          status: 200,
          contentType: 'application/json',
          body: JSON.stringify({
            downloads: [
              {
                key: 'llamacpp@b9999@cpu',
                engine: 'llamacpp',
                backend: 'cpu',
                version: 'b9999',
                status: 'downloading',
                task_id: '11111111-1111-1111-1111-111111111111',
                bytes_received: 1024,
                total_bytes: 4096,
                percent: 25,
              },
            ],
          }),
        })
      },
    )

    // First mount → the store's loadActive() init fires once.
    await gotoRuntimeSettings(page, baseURL)
    await expect.poll(() => listCalls, { timeout: 15000 }).toBeGreaterThan(0)
    const afterFirstMount = listCalls

    // Reload → the app re-mounts, the store re-inits, and loadActive() runs
    // again, rehydrating the in-flight download.
    await page.reload()
    await expect(page.getByRole('tab', { name: 'Llama.cpp' })).toBeVisible({
      timeout: 30000,
    })
    await expect
      .poll(() => listCalls, { timeout: 15000 })
      .toBeGreaterThan(afterFirstMount)
  })
})
