import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { byTestId } from '../testid'
import {
  openScheduleDialog,
  seedModelAndConversation,
  switchSegment,
} from './chat-schedule-helpers'

/**
 * TEST-85 (ITEM-20) — the merged in-chat dialog switches between its modes.
 *
 * The dialog exposes THREE selectable schedule modes in-chat (DEC-20 "merge
 * /schedule + /loop"):
 *   1. Schedule → Once      (a fixed `run_at`)
 *   2. Schedule → Recurring (a cron)
 *   3. Loop (self-paced)    (`schedule_kind='self_paced'`, decides its own cadence)
 *
 * The top-level mode is the `schedule-loop-mode` Segmented (Schedule / Loop); the
 * Once/Recurring sub-mode is the reused `ScheduleBuilder`'s `schedule-kind`
 * Segmented. This proves each mode is reachable and swaps the mode-specific
 * controls in/out — the pure-UI half of the assert. (The "live Test button shows
 * real output each mode" half is NOT built into this in-chat dialog — see report.)
 */
test.describe('In-chat schedule/loop dialog — mode switching (ITEM-20)', () => {
  test('all three modes are selectable and swap their mode-specific controls', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const seed = await seedModelAndConversation(page, apiURL)
    await openScheduleDialog(page, baseURL, seed.conversationId)

    // Default: Schedule mode, Recurring sub-mode → the cron builder is shown, and
    // the loop-only completion field is absent.
    await expect(byTestId(page, 'schedule-kind')).toBeVisible({ timeout: 10000 })
    await expect(byTestId(page, 'schedule-preset')).toBeVisible()
    await expect(byTestId(page, 'schedule-loop-completion')).toHaveCount(0)

    // (Segmented is a base-ui Tabs control whose raised active-pill layer fools
    // Playwright's pointer hit-test; `switchSegment` performs the hover+force-click
    // sequence proven to flip the segment. The assertions still prove the switch.)

    // Mode 1 — Schedule → Once: the datetime input replaces the cron builder.
    await switchSegment(page, 'schedule-kind-opt-once')
    await expect(byTestId(page, 'schedule-run-at')).toBeVisible({ timeout: 10000 })
    await expect(byTestId(page, 'schedule-preset')).toHaveCount(0)

    // Mode 2 — Schedule → Recurring: back to the cron builder.
    await switchSegment(page, 'schedule-kind-opt-recurring')
    await expect(byTestId(page, 'schedule-preset')).toBeVisible({ timeout: 10000 })
    await expect(byTestId(page, 'schedule-run-at')).toHaveCount(0)

    // Mode 3 — Loop (self-paced): the whole ScheduleBuilder is hidden and the
    // self-paced "Stop when…" + explanatory note appear (schedule_kind='self_paced').
    await switchSegment(page, 'schedule-loop-mode-opt-loop')
    await expect(byTestId(page, 'schedule-loop-completion')).toBeVisible({
      timeout: 10000,
    })
    await expect(byTestId(page, 'schedule-loop-selfpaced-note')).toBeVisible()
    await expect(byTestId(page, 'schedule-kind')).toHaveCount(0)

    // …and back to Schedule restores the builder (the modes are truly toggleable).
    await switchSegment(page, 'schedule-loop-mode-opt-schedule')
    await expect(byTestId(page, 'schedule-kind')).toBeVisible({ timeout: 10000 })
    await expect(byTestId(page, 'schedule-loop-completion')).toHaveCount(0)
  })
})
