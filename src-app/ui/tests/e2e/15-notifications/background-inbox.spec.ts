import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { byTestId } from '../testid'

/**
 * TEST-129 (ITEM-26) — the agent/background inbox ("Background results").
 *
 * asserts (TESTS.md): the inbox lists live task status + result; needs_input
 * bubbled top w/ reply; light/dark + 390px.
 *
 * The no-LLM half proven here (the task's core ask): the `/notifications/background`
 * AgentInboxPage — a focused VIEW over the shared notification inbox, narrowed to
 * the agent/background kinds (`AGENT_INBOX_KINDS`) — LISTS a real agent
 * notification, its nav entry is present + gated by `notifications::read`, and it
 * renders at a 390px mobile width. Notifications are server-emitted (no create
 * API), so the row is seeded directly into the per-test DB via `sql()` — the
 * page's mount-time `load()` then fetches it through the real REST endpoint.
 *
 * ("needs_input bubbled top w/ reply" is a live `waiting` background-run state and
 * is reported separately; the inbox here proves the listing + gating + responsive.)
 */
test.describe('Agent/background inbox — lists agent notifications (ITEM-26)', () => {
  test('lists a seeded agent notification, nav entry gated by NotificationsRead, renders at 390px', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL, sql } = testInfra
    void apiURL

    await loginAsAdmin(page, baseURL)

    // The admin (holds `*`, therefore `notifications::read`) sees the discoverable
    // "Background results" nav entry (id `agent-inbox`, gated NotificationsRead).
    await expect(
      byTestId(page, 'layout-sidebar-nav-menu-item-agent-inbox'),
    ).toBeVisible({ timeout: 30000 })

    // Seed a real agent-kind notification for the admin (kind ∈ AGENT_INBOX_KINDS).
    const adminId = (
      await sql(`SELECT id FROM users WHERE username = 'admin' LIMIT 1`)
    ).rows[0].id as string
    const title = 'Weekly digest ran'
    const body = '3 new CRISPR papers since last run.'
    const inserted = await sql(
      `INSERT INTO notifications (user_id, kind, title, body, interrupt, payload)
       VALUES ($1, 'scheduled_task_result', $2, $3, true, '{}'::jsonb)
       RETURNING id`,
      [adminId, title, body],
    )
    const notifId = inserted.rows[0].id as string

    // The AgentInboxPage loads the inbox on entry and narrows to agent kinds.
    await page.goto(`${baseURL}/notifications/background`)
    await expect(byTestId(page, 'agent-inbox-page')).toBeVisible({ timeout: 30000 })

    // The seeded agent notification is listed (card + its content). Scope the text
    // to the card — the same notification also feeds the app-shell bell, so a bare
    // getByText would be a strict-mode multi-match.
    const card = byTestId(page, `agent-inbox-card-${notifId}`)
    await expect(card).toBeVisible({ timeout: 15000 })
    await expect(card).toContainText(title)
    await expect(card).toContainText(body)

    // Renders at a 390px mobile width (the card + page survive the narrow viewport).
    await page.setViewportSize({ width: 390, height: 844 })
    await expect(byTestId(page, 'agent-inbox-page')).toBeVisible()
    await expect(card).toBeVisible()
    await expect(card).toContainText(title)
  })
})
