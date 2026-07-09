import { expect, test } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { byTestId } from '../testid'

/**
 * E2E — Notification inbox: a background result renders in the inbox and
 * mark-read clears its unread state. Mocks the notification REST endpoints at
 * the HTTP boundary.
 */

const NID = '33333333-3333-3333-3333-333333333333'

function notif(readAt: string | null) {
  return {
    id: NID,
    user_id: '00000000-0000-0000-0000-000000000001',
    kind: 'scheduled_task_result',
    title: 'Weekly digest ran',
    body: '3 new since last run.',
    interrupt: true,
    scheduled_task_id: null,
    workflow_run_id: null,
    conversation_id: null,
    read_at: readAt,
    created_at: '2026-07-09T09:00:00Z',
  }
}

test('inbox shows a notification and mark-read clears unread', async ({ page, baseURL }) => {
  let read = false

  await page.route(/\/api\/notifications(\?.*)?$/, async route => {
    const n = notif(read ? '2026-07-09T09:05:00Z' : null)
    await route.fulfill({
      status: 200,
      json: { items: [n], total: 1, unread: read ? 0 : 1, page: 1, per_page: 30 },
    })
  })
  await page.route(/\/api\/notifications\/[^/]+\/read$/, async route => {
    read = true
    await route.fulfill({ status: 200, json: { unread: 0 } })
  })
  await page.route(/\/api\/notifications\/unread-count$/, async route =>
    route.fulfill({ status: 200, json: { unread: read ? 0 : 1 } }),
  )

  await loginAsAdmin(page, baseURL as string)
  await page.goto(`${baseURL}/notifications`)

  // The notification renders.
  await expect(byTestId(page, `notification-card-${NID}`)).toBeVisible({ timeout: 10000 })
  await expect(page.getByText('Weekly digest ran')).toBeVisible()

  // Mark it read.
  await byTestId(page, `notification-read-${NID}`).click()

  // The per-row read button disappears once read (read_at set on refetch).
  await expect(byTestId(page, `notification-read-${NID}`)).toBeHidden({ timeout: 10000 })
})
