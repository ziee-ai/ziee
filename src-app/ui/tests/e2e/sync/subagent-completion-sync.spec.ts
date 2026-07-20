import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'
import { loginAsAdmin, getCurrentUserToken } from '../../common/auth-helpers'
import {
  seedBridgeConversation,
  spawnBackgroundSubagent,
  HAS_BRIDGE,
  BRIDGE_SKIP,
} from '../chat/helpers/agent-llm-helpers'

/**
 * TEST-22 / ITEM-9 — results-land-when-done: a background sub-agent completing on
 * device A pushes its completion NOTIFICATION to the SAME user's device B live,
 * with NO manual reload.
 *
 * A real detached sub-agent run is launched through the production
 * `spawn_background` path (real bridge turn). On COMPLETION the runner posts a
 * durable `background_run_result` notification and emits `SyncEntity::Notification`
 * (owner audience, origin=None). Device B — already sitting on the agent/background
 * inbox (`/notifications/background`), whose SDK `Notifications` store subscribes to
 * `sync:notification` + refetches — surfaces the "Background task finished" card
 * live. Device B is the away device; device A launched the work.
 *
 * Requires the agent-core chat path + a real LLM bridge. Skips cleanly when unset.
 * --workers=1.
 */
test.describe('realtime sync — background sub-agent completion notification', () => {
  test.skip(!HAS_BRIDGE, BRIDGE_SKIP)
  test.setTimeout(150_000)

  test('a background run completing on device A surfaces its notification on device B live', async ({
    page,
    browser,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getCurrentUserToken(page)

    const { conversationId } = await seedBridgeConversation(
      page,
      apiURL,
      token,
      'Completion Sync Model',
    )

    // Device B: same user, sitting on the agent/background inbox BEFORE the run is
    // launched — so the live sync delivery is what surfaces the completion.
    const ctxB = await browser.newContext()
    const pageB = await ctxB.newPage()
    try {
      await loginAsAdmin(pageB, baseURL)
      await pageB.goto(`${baseURL}/notifications/background`)
      await expect(byTestId(pageB, 'agent-inbox-page')).toBeVisible({ timeout: 30_000 })

      // Device A launches a REAL detached sub-agent on a trivial task that
      // completes quickly (so the run reaches `completed` → posts + emits the
      // completion notification).
      await spawnBackgroundSubagent(
        page,
        apiURL,
        token,
        conversationId,
        'Reply with the single word DONE and nothing else.',
      )

      // Device B surfaces the completion notification card LIVE (sync:notification
      // → SDK store refetch), no manual reload. Scope to the inbox page so the
      // shared app-shell bell (same notification) isn't a strict-mode multi-match.
      await expect(
        byTestId(pageB, 'agent-inbox-page').getByText('Background task finished').first(),
      ).toBeVisible({ timeout: 120_000 })

      // The empty state is gone — a real notification landed via sync.
      await expect(byTestId(pageB, 'agent-inbox-empty')).toHaveCount(0)
    } finally {
      await ctxB.close()
    }
  })
})
