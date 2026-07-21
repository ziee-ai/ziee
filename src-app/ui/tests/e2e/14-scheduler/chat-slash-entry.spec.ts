import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { byTestId } from '../testid'
import { seedModelAndConversation } from './chat-schedule-helpers'

/**
 * TEST-83 (ITEM-19) — the schedule/loop entry point.
 *
 * asserts (TESTS.md): IF a slash entry is built, `/schedule` + `/loop` open the
 * SAME dialog; guarded/skipped when disabled (default button-only).
 *
 * The shipped design is DEFAULT BUTTON-ONLY (DEC-41 "toolbar button is the sole
 * entry"; the chat module has no slash-command system). This spec proves exactly
 * that, without skipping: typing `/schedule` or `/loop` into the composer does NOT
 * open the dialog (there is no slash entry), while the composer toolbar button —
 * the sole entry point — DOES open it and it is the SAME merged dialog for both
 * schedule and loop (mode is chosen inside it).
 */
test.describe('Schedule/loop entry point is button-only (ITEM-19)', () => {
  test('slash text does not open the dialog; the toolbar button is the sole entry', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const seed = await seedModelAndConversation(page, apiURL)

    await page.goto(`${baseURL}/chat/${seed.conversationId}`)
    await page.waitForLoadState('load')
    const composer = page.locator('textarea[placeholder*="Type your message"]')
    await expect(composer).toBeVisible({ timeout: 30000 })

    // No slash entry: typing "/schedule" / "/loop" is plain text — no dialog opens.
    await composer.fill('/schedule')
    await expect(byTestId(page, 'schedule-loop-form')).toHaveCount(0)
    await composer.fill('/loop')
    await expect(byTestId(page, 'schedule-loop-form')).toHaveCount(0)
    await composer.fill('')

    // The toolbar button is the sole entry point, and it opens the SAME merged
    // dialog (which itself offers both Schedule and Loop modes — ITEM-20).
    const button = byTestId(page, 'chat-schedule-loop-button')
    await expect(button).toBeVisible({ timeout: 15000 })
    await expect(button).toBeEnabled({ timeout: 15000 })
    await button.click()
    await expect(byTestId(page, 'schedule-loop-form')).toBeVisible({
      timeout: 15000,
    })
    // Both modes are reachable from this one dialog (the "same dialog" invariant).
    await expect(byTestId(page, 'schedule-loop-mode-opt-schedule')).toBeVisible()
    await expect(byTestId(page, 'schedule-loop-mode-opt-loop')).toBeVisible()
  })
})
