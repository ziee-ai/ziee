// Run with --workers=1 (mandated for all E2E here): parallel workers share the
// backend + test DB and race.
import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'

/**
 * frontend-07 / cross-cutting-05 — conversation summarizer threshold validation.
 *
 * The summarizer fields were renamed to token units
 * (`summarize_after_tokens` / `summarizer_keep_recent_tokens`) and gained a
 * client-side validator rejecting `keep >= trigger` (mirrors the migration-85
 * DB CHECK). None of the existing 12-memory specs exercised the summarizer
 * form. This spec drives the real admin page:
 *
 *   - validation path: trigger 5000, keep 6000 → inline error
 *     "Keep-recent (6000) must be less than the trigger (5000)." + NO success
 *     toast (the form never submits).
 *   - happy path: trigger 20000, keep 4000 → "Summarizer settings saved." toast.
 *
 * Field labels come from SummarizerSection.tsx ("Summarize after N tokens" /
 * "Keep recent tokens verbatim"); the submit is the section's primary Save.
 */

// Exact label strings from SummarizerSection.tsx Form.Item `label`s.
const TRIGGER_LABEL = 'Summarize after N tokens'
const KEEP_LABEL = 'Keep recent tokens verbatim'
const SUCCESS_TOAST = 'Summarizer settings saved.'

/**
 * The summarizer Card. The page stacks several admin cards, each with its own
 * "Save" — scope every interaction to the card whose head-title is
 * "Conversation summarizer" so we never click a sibling section's Save. The
 * field labels are unique to this section, but the Save text is not. AntD Card
 * titles render in `.ant-card-head-title` (a div, NOT a heading role), so we
 * match that, mirroring the existing 05-llm card-scoping pattern.
 */
function summarizerCard(page: import('@playwright/test').Page) {
  return page.locator(
    '.ant-card:has(.ant-card-head-title:has-text("Conversation summarizer"))',
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

test.describe('Memory — summarizer thresholds', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    await loginAsAdmin(page, testInfra.baseURL)
    await page.goto(`${testInfra.baseURL}/settings/memory-admin`)
    // The "Conversation summarizer" card renders once admin settings load.
    // AntD Card titles are `.ant-card-head-title` divs, not headings.
    await expect(
      page.locator(
        '.ant-card-head-title:has-text("Conversation summarizer")',
      ),
    ).toBeVisible({ timeout: 30000 })
  })

  test('rejects keep >= trigger with an inline error and no success toast', async ({
    page,
  }) => {
    const card = summarizerCard(page)
    const trigger = card.getByLabel(TRIGGER_LABEL)
    const keep = card.getByLabel(KEEP_LABEL)

    // keep (6000) >= trigger (5000) — invalid.
    await setNumberField(trigger, 5000)
    await setNumberField(keep, 6000)

    // The "Keep recent" Form.Item revalidates on its `summarize_after_tokens`
    // dependency; submitting forces the validator to run. Scope the submit to
    // this card so a sibling section's Save is never the target.
    await card.locator('.ant-btn-primary[type="submit"]').click()

    // Inline validation error appears with the exact interpolated copy.
    await expect(
      card.locator('.ant-form-item-explain-error').filter({
        hasText: 'Keep-recent (6000) must be less than the trigger (5000).',
      }),
    ).toBeVisible({ timeout: 10000 })

    // The form never submitted → no success toast.
    await expect(page.getByText(SUCCESS_TOAST)).toHaveCount(0)
  })

  test('saves a valid trigger/keep pair', async ({ page }) => {
    const card = summarizerCard(page)
    const trigger = card.getByLabel(TRIGGER_LABEL)
    const keep = card.getByLabel(KEEP_LABEL)

    // keep (4000) < trigger (20000) — valid.
    await setNumberField(trigger, 20000)
    await setNumberField(keep, 4000)

    await card.locator('.ant-btn-primary[type="submit"]').click()

    // Success toast confirms the PUT landed.
    await expect(page.getByText(SUCCESS_TOAST)).toBeVisible({ timeout: 10000 })
    // And no validation error is shown in this card.
    await expect(card.locator('.ant-form-item-explain-error')).toHaveCount(0)
  })
})
