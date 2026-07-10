import { expect, test } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { byTestId } from '../testid'

/**
 * E2E — Scheduler admin settings (ITEM-24): an admin opens `/settings/scheduler`,
 * edits the quota + retention, saves, and the new values persist (the store
 * re-reads them). Mocks the admin-settings REST endpoint at the HTTP boundary.
 */

function settings(overrides: Partial<Record<string, number>> = {}) {
  return {
    max_active_tasks_per_user: 20,
    min_interval_seconds: 300,
    max_consecutive_failures: 5,
    notification_retention_days: 30,
    updated_at: '2026-07-09T00:00:00Z',
    ...overrides,
  }
}

test('admin edits quota + retention and it persists', async ({ page, testInfra }) => {
  const { baseURL } = testInfra
  let current = settings()

  await page.route(/\/api\/scheduler\/admin-settings$/, async (route, req) => {
    if (req.method() === 'PUT') {
      const body = req.postDataJSON() as Record<string, number>
      current = settings(body)
      await route.fulfill({ status: 200, json: current })
      return
    }
    await route.fulfill({ status: 200, json: current })
  })

  await loginAsAdmin(page, baseURL)
  await page.goto(`${baseURL}/settings/scheduler`)

  await expect(byTestId(page, 'scheduler-admin-page')).toBeVisible({ timeout: 10000 })

  // Edit the quota + retention. The kit InputNumber forwards data-testid to the
  // input element itself, so target it directly (with a small nested fallback).
  const maxActive = byTestId(page, 'scheduler-max-active')
  await maxActive.fill('42').catch(() => maxActive.locator('input').fill('42'))
  const retention = byTestId(page, 'scheduler-retention')
  await retention.fill('7').catch(() => retention.locator('input').fill('7'))

  await byTestId(page, 'scheduler-admin-save').click()

  // Persisted: the PUT captured the new value.
  await expect.poll(() => current.max_active_tasks_per_user).toBe(42)
  await expect.poll(() => current.notification_retention_days).toBe(7)
})
