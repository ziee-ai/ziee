// Run with --workers=1 (mandated for all E2E here): parallel workers share the
// backend + test DB and race.
import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'

/**
 * E2E — Summarization admin settings page (migration 91 extraction).
 *
 * Retargeted from the deleted `12-memory/summarizer-thresholds.spec.ts`
 * to the new dedicated `/settings/summarization-admin` page. The form
 * fields kept the same labels ("Summarize after N tokens" / "Keep
 * recent tokens verbatim") but the page is now its own settings card
 * + standalone route.
 *
 *   - validation path: trigger 5000, keep 6000 → inline error
 *     "Keep-recent (6000) must be less than the trigger (5000)." + NO
 *     success toast (the form's pre-submit validator never lets the PUT
 *     fire).
 *   - happy path: trigger 20000, keep 4000 → "Summarization settings
 *     saved." toast.
 */

const TRIGGER_LABEL = 'Summarize after N tokens'
const KEEP_LABEL = 'Keep recent tokens verbatim'
const SUCCESS_TOAST = 'Summarization settings saved.'

/**
 * The settings card. The page only renders one card today
 * ("Summarization"), but scoping defends against future neighbors and
 * keeps the selector style consistent with the 12-memory tests.
 */
function summarizationCard(page: import('@playwright/test').Page) {
  return page.locator(
    '.ant-card:has(.ant-card-head-title:has-text("Summarization"))',
  )
}

/** Set an antd InputNumber addressed by its Form.Item label to `value`. */
async function setNumberField(
  field: import('@playwright/test').Locator,
  value: number,
) {
  await field.click()
  await field.press('ControlOrMeta+a')
  await field.fill(String(value))
  // Commit the value (antd InputNumber formats on blur / Enter).
  await field.press('Enter')
}

test.describe('Summarization — admin thresholds', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    await loginAsAdmin(page, testInfra.baseURL)
    await page.goto(`${testInfra.baseURL}/settings/summarization-admin`)
    await expect(
      page.locator('.ant-card-head-title:has-text("Summarization")'),
    ).toBeVisible({ timeout: 30000 })
  })

  test('rejects keep >= trigger with an inline error and no success toast', async ({
    page,
  }) => {
    const card = summarizationCard(page)
    const trigger = card.getByLabel(TRIGGER_LABEL)
    const keep = card.getByLabel(KEEP_LABEL)

    // keep (6000) >= trigger (5000) — invalid.
    await setNumberField(trigger, 5000)
    await setNumberField(keep, 6000)

    await card.locator('.ant-btn-primary[type="submit"]').click()

    await expect(
      page
        .getByText('Keep-recent (6000) must be less than the trigger (5000).')
        .first(),
    ).toBeVisible({ timeout: 10000 })

    // The form never submitted → no success toast.
    await expect(page.getByText(SUCCESS_TOAST)).toHaveCount(0)
  })

  test('saves a valid trigger/keep pair', async ({ page }) => {
    const card = summarizationCard(page)
    const trigger = card.getByLabel(TRIGGER_LABEL)
    const keep = card.getByLabel(KEEP_LABEL)

    // keep (4000) < trigger (20000) — valid.
    await setNumberField(trigger, 20000)
    await setNumberField(keep, 4000)

    await card.locator('.ant-btn-primary[type="submit"]').click()

    await expect(page.getByText(SUCCESS_TOAST).first()).toBeVisible({
      timeout: 10000,
    })
  })
})
