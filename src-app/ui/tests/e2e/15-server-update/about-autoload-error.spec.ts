import type { Page } from '@playwright/test'
import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { byTestId } from '../testid'

// ---------------------------------------------------------------------------
// AboutSettings AUTO-LOAD error path (audit all-abe941c1b486).
//
// The existing `surfaces an error when the status endpoint fails` test only
// asserts the error Alert AFTER clicking Refresh. But the ServerUpdate store's
// `__init__` eager-loads via `loadStatus()` on first access, so the About page
// must surface a failed initial fetch on MOUNT — with no Refresh click. This
// drives that on-mount failure path and then asserts recovery once the endpoint
// is healthy again. Only the /status HTTP boundary is mocked; the store's
// error-state machine and the Alert render are real.
// ---------------------------------------------------------------------------

const STATUS_OK = {
  current_version: '0.1.0',
  latest_version: '0.1.0',
  update_available: false,
  release_url: null,
  notes: null,
  checked_at: '2026-06-12T00:00:00Z',
  enabled: true,
}

test.describe('Server update — About auto-load error', () => {
  test('error Alert renders on mount when the initial status fetch fails (no Refresh)', async ({
    page,
    testInfra,
  }: {
    page: Page
    testInfra: { baseURL: string }
  }) => {
    const { baseURL } = testInfra

    // Fail the status endpoint BEFORE any navigation, so the store's on-mount
    // eager `loadStatus()` (the __init__ auto-load) hits the 500.
    let failing = true
    await page.route(/\/api\/server-update\/status$/, async route => {
      if (failing) {
        await route.fulfill({
          status: 500,
          contentType: 'application/json',
          body: '{}',
        })
      } else {
        await route.fulfill({
          status: 200,
          contentType: 'application/json',
          body: JSON.stringify(STATUS_OK),
        })
      }
    })

    await loginAsAdmin(page, baseURL)
    await page.goto(`${baseURL}/settings/about`)

    // Auto-load failure surfaces the error Alert WITHOUT clicking Refresh.
    await expect(byTestId(page, 'serverupd-error-alert')).toBeVisible({ timeout: 30000 })

    // Recovery: with the endpoint healthy, Reload status clears the error and
    // renders the version, proving the error state was transient (not latched).
    failing = false
    await byTestId(page, 'serverupd-refresh-btn').click()
    await expect(byTestId(page, 'serverupd-error-alert')).toHaveCount(0, {
      timeout: 30000,
    })
    // '0.1.0' is dynamic data from the mocked /status payload — assert it
    // inside the About descriptions block (testid scope + data-value text).
    await expect(byTestId(page, 'serverupd-about-descriptions')).toContainText('0.1.0', {
      timeout: 30000,
    })
  })
})
