import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'

/**
 * DownloadIndicatorWidget (sidebarBottom slot). The widget self-hides when
 * there are no active/failed downloads, so we mock the downloads list endpoint
 * to surface one in-flight download, then exercise the full popover-open
 * interaction (badge → click → "Downloads" popover listing the active item).
 */
test.describe('LLM Providers - download indicator widget', () => {
  test('an active download surfaces the badge + opens the downloads popover', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)

    // The widget's store self-inits from GET /api/llm-models/downloads.
    await page.route(/\/api\/llm-models\/downloads(\?.*)?$/, async (route, req) => {
      if (req.method() === 'GET') {
        return route.fulfill({
          status: 200,
          contentType: 'application/json',
          body: JSON.stringify({
            downloads: [
              {
                id: '11111111-1111-1111-1111-111111111111',
                provider_id: '22222222-2222-2222-2222-222222222222',
                repository_id: '33333333-3333-3333-3333-333333333333',
                status: 'downloading',
                created_at: '2026-06-01T00:00:00Z',
                started_at: '2026-06-01T00:00:00Z',
                updated_at: '2026-06-01T00:00:00Z',
                request_data: {
                  model_name: 'tiny-test-model',
                  display_name: 'Tiny Test Model',
                  repository_path: 'org/tiny-test-model',
                },
                progress_data: {
                  current: 50,
                  total: 100,
                  eta_seconds: 30,
                  message: 'downloading',
                  phase: 'downloading',
                  speed_bps: 1000,
                },
              },
            ],
            page: 1,
            per_page: 50,
            total: 1,
          }),
        })
      }
      return route.continue()
    })

    // Land on the app shell — the sidebarBottom widget mounts + loads.
    await page.goto(`${baseURL}/`)

    // The badge appears (one active download). Click the indicator to open the
    // popover.
    const indicator = page.locator('.anticon-download').first()
    await expect(indicator).toBeVisible({ timeout: 30000 })
    await indicator.click()

    // The "Downloads" popover lists the in-flight download.
    await expect(page.getByText('Downloads')).toBeVisible({ timeout: 10000 })
    await expect(page.getByText('Active Downloads (1)')).toBeVisible()
    await expect(page.getByText('Tiny Test Model')).toBeVisible()
  })
})
