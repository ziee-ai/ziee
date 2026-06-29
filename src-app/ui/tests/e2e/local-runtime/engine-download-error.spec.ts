import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { byTestId } from '../testid.ts'
import { gotoRuntimeSettings } from './helpers/local-runtime-helpers'

/**
 * E2E — engine-download ERROR path (AvailableVersionsCard.tsx handleDownload
 * catch → message.error).
 *
 * Audit gap: engine-lifecycle covers only the happy-path Install (and is
 * gated on real network + HUGGINGFACE_API_KEY). This mocks the two HTTP
 * boundaries: check-updates returns one installable version (so the Install
 * button renders) and the download POST fails (500). Clicking Install must
 * surface the error toast — exercising the failure branch deterministically,
 * no network needed.
 */

const CHECK_UPDATES = '**/api/local-runtime/versions/*/check-updates'
const DOWNLOAD = '**/api/local-runtime/versions/download'

test.describe('Local Runtime — engine download error', () => {
  test('a failed download POST surfaces an error toast', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)

    // check-updates → one ready, not-installed version per engine.
    await page.route(CHECK_UPDATES, async route => {
      const m = route.request().url().match(/versions\/([^/]+)\/check-updates/)
      const engine = m ? decodeURIComponent(m[1]) : 'llamacpp'
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({
          engine,
          platform: 'linux',
          arch: 'x86_64',
          versions: [
            {
              version: 'v9.9.9-test',
              binary_ready: true,
              installed: false,
              available_backends: ['cpu'],
              installed_backends: [],
              prerelease: false,
              recommended_backend: 'cpu',
            },
          ],
        }),
      })
    })

    // The download POST fails.
    await page.route(DOWNLOAD, async route => {
      if (route.request().method() !== 'POST') return route.fallback()
      await route.fulfill({
        status: 500,
        contentType: 'application/json',
        body: JSON.stringify({ message: 'Download server unavailable' }),
      })
    })

    await gotoRuntimeSettings(page, baseURL)

    const installBtn = byTestId(page, 'llmrt-version-install-v9.9.9-test')
    await expect(installBtn).toBeVisible({ timeout: 30000 })
    await installBtn.click()

    // The failed POST surfaces an error toast.
    await expect(page.locator('[data-sonner-toast][data-type="error"]')).toBeVisible({
      timeout: 15000,
    })
  })
})
