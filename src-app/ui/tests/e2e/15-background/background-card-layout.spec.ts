import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import { byTestId } from '../testid'
import {
  adminUserId,
  openTasksPanel,
  seedConversationRun,
  seedConversationWithMessage,
} from './helpers/background-helpers'

/**
 * TEST-1 (ITEM-1 / ITEM-2, acceptance for INV-1) — the Tasks panel cards are NOT
 * clipped. The regression: the kit `Card` is `overflow-hidden`, so as a flex
 * child in the panel's `flex h-full flex-col` column it computed `min-height:0`
 * and SHRANK — a card whose content was 152px was rendered at 58px, cutting off
 * the meta row (kind · time) and the actions on EVERY card. `shrink-0` fixes it.
 *
 * The proof is behavioural, at a POPULATED panel and at BOTH desktop and 390px:
 * for each card, `scrollHeight === offsetHeight` (nothing clipped), the kind tag
 * and the details toggle are visible, and the page never scrolls horizontally.
 */

type Sql = (
  text: string,
  params?: unknown[],
) => Promise<{ rows: Record<string, unknown>[] }>

/** Seed enough completed runs that the list overflows the panel — the exact
 *  condition under which the pre-fix flex column shrank each card. */
async function seedManyCompleted(
  sql: Sql,
  userId: string,
  conversationId: string,
  n: number,
): Promise<void> {
  for (let i = 0; i < n; i++) {
    const runId = await seedConversationRun(sql, userId, conversationId, {
      status: 'completed',
      task: `Task number ${i} with a deliberately long-ish label to exercise the two-line title clamp`,
    })
    // Give it a result body so the card renders its full content (result-ready
    // tag + details toggle) — the tallest state, i.e. the worst case for clipping.
    await sql(`UPDATE workflow_runs SET final_output_json = $2::jsonb WHERE id = $1`, [
      runId,
      JSON.stringify({ final_text: `Answer ${i}`, tokens_used: 100 + i }),
    ])
  }
}

async function assertNoCardsClipped(page: import('@playwright/test').Page): Promise<void> {
  const cards = page.locator('[data-testid^="background-run-card-"]')
  const count = await cards.count()
  expect(count).toBeGreaterThan(3)
  for (let i = 0; i < count; i++) {
    const card = cards.nth(i)
    // Nothing clipped: the card renders at its natural content height.
    const metrics = await card.evaluate(el => ({
      offsetH: (el as HTMLElement).offsetHeight,
      scrollH: (el as HTMLElement).scrollHeight,
    }))
    expect(
      metrics.scrollH,
      `card #${i} clipped: scrollHeight ${metrics.scrollH} > offsetHeight ${metrics.offsetH}`,
    ).toBeLessThanOrEqual(metrics.offsetH + 1)
  }
  // The kind tag + details toggle on the FIRST card are actually visible (not
  // merely present-but-clipped).
  const firstId = await cards
    .first()
    .getAttribute('data-testid')
    .then(v => v!.replace('background-run-card-', ''))
  await expect(byTestId(page, `background-run-kind-${firstId}`)).toBeVisible()
  await expect(byTestId(page, `background-run-details-toggle-${firstId}`)).toBeVisible()
}

test.describe('background Tasks panel — card layout (ITEM-1/ITEM-2)', () => {
  test('cards are not clipped at desktop OR 390px, with no horizontal page scroll', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL, sql } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    const userId = await adminUserId(sql)

    const conv = await seedConversationWithMessage(page, apiURL, token, sql, 'Layout chat')
    await seedManyCompleted(sql, userId, conv, 8)

    // ── desktop ──────────────────────────────────────────────────────────────
    await page.setViewportSize({ width: 1280, height: 900 })
    await openTasksPanel(page, baseURL, conv)
    await expect(byTestId(page, 'background-panel-list')).toBeVisible({ timeout: 15_000 })
    await assertNoCardsClipped(page)

    // ── mobile (390px) ───────────────────────────────────────────────────────
    await page.setViewportSize({ width: 390, height: 844 })
    // Re-open the panel from the footer at the narrow width (the right panel may
    // reflow); the panel list must be present again.
    await openTasksPanel(page, baseURL, conv)
    await expect(byTestId(page, 'background-panel-list')).toBeVisible({ timeout: 15_000 })
    await assertNoCardsClipped(page)

    // No horizontal PAGE scroll at 390px.
    const overflow = await page.evaluate(() => {
      const el = document.scrollingElement || document.documentElement
      return el.scrollWidth - el.clientWidth
    })
    expect(overflow, `horizontal page overflow of ${overflow}px at 390px`).toBeLessThanOrEqual(1)
  })
})
