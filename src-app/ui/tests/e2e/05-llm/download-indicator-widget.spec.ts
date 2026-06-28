import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'

/**
 * E2E — DownloadIndicatorWidget popover (sidebarBottom slot,
 * DownloadIndicatorWidget.tsx). The widget renders only when there are active
 * or failed downloads, and a click opens a Popover listing them. The downloads
 * LIST endpoint is the external boundary — mocked to return one active download
 * so the widget renders deterministically; the badge + popover interaction is
 * the behavior under test.
 */

test.describe('LLM — download indicator widget', () => {
  test('an active download renders the badge and the popover lists it', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    const now = new Date().toISOString()
    const downloadingRow = {
      id: 'dddddddd-1111-2222-3333-444444444444',
      provider_id: 'pppppppp-1111-2222-3333-444444444444',
      repository_id: 'huggingface',
      status: 'downloading',
      created_at: now,
      started_at: now,
      updated_at: now,
      request_data: { model_name: 'widget-model', display_name: 'WidgetIndicatorModel' },
      progress_data: {
        current: 1_000_000,
        total: 50_000_000,
        phase: 'downloading',
        message: '',
        speed_bps: 400_000,
        eta_seconds: 120,
      },
    }

    await page.route(/\/api\/llm-models\/downloads(\?.*)?$/, async route => {
      if (route.request().method() !== 'GET') return route.fallback()
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({ downloads: [downloadingRow], page: 1, per_page: 100, total: 1 }),
      })
    })

    await loginAsAdmin(page, baseURL)
    await page.goto(`${baseURL}/chats`)
    // Reload so the downloads list fetch fires under the mock.
    await page.reload()

    // The widget's download icon renders in the sidebar bottom.
    const icon = page.locator('.anticon-download').first()
    await expect(icon).toBeVisible({ timeout: 30000 })

    // Click it → the Downloads popover opens and lists the active download.
    await icon.click()
    const popover = page.locator('.ant-popover').filter({ hasText: 'Downloads' })
    await expect(popover).toBeVisible({ timeout: 10000 })
    await expect(popover.getByText(/Active Downloads \(1\)/)).toBeVisible()
    await expect(popover.getByText('WidgetIndicatorModel')).toBeVisible()
  })
})
