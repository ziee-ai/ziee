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

  test('rejects full_summary_prompt missing the {transcript} placeholder', async ({
    page,
  }) => {
    const card = summarizationCard(page)
    const fullPrompt = card.getByLabel('Full-summary prompt')
    // Non-empty value that lacks `{transcript}` — backend (and the
    // pre-submit validator) must reject so the engine never gets a
    // template it can't interpolate.
    await fullPrompt.click()
    await fullPrompt.press('ControlOrMeta+a')
    await fullPrompt.fill('Summarize this conversation, please.')
    await card.locator('.ant-btn-primary[type="submit"]').click()

    // Inline error from the form's pre-submit validator OR a backend
    // 400 surfaced as a message — match on the placeholder name.
    await expect(
      page.getByText(/\{transcript\}/i).first(),
    ).toBeVisible({ timeout: 10000 })
    await expect(page.getByText(SUCCESS_TOAST)).toHaveCount(0)
  })

  test('toggling the "Enable summarization" Switch persists across reload', async ({
    page,
    testInfra,
  }) => {
    const card = summarizationCard(page)
    const toggle = card.getByRole('switch', {
      name: 'Enable summarization deployment-wide',
    })
    await expect(toggle).toBeVisible({ timeout: 10000 })

    const before = await toggle.getAttribute('aria-checked')
    await toggle.click()
    const after = await toggle.getAttribute('aria-checked')
    expect(after).not.toBe(before)

    await card.locator('.ant-btn-primary[type="submit"]').click()
    await expect(page.getByText(SUCCESS_TOAST).first()).toBeVisible({
      timeout: 10000,
    })

    // Reload — the persisted value must come back (the PUT actually fired).
    await page.goto(`${testInfra.baseURL}/settings/summarization-admin`)
    const reloaded = summarizationCard(page).getByRole('switch', {
      name: 'Enable summarization deployment-wide',
    })
    await expect(reloaded).toHaveAttribute('aria-checked', after!, {
      timeout: 10000,
    })

    // Restore the original state so re-runs start from a known baseline.
    await reloaded.click()
    await summarizationCard(page)
      .locator('.ant-btn-primary[type="submit"]')
      .click()
    await expect(page.getByText(SUCCESS_TOAST).first()).toBeVisible({
      timeout: 10000,
    })
  })

  test('saves valid prompt overrides', async ({ page }) => {
    const card = summarizationCard(page)
    const fullPrompt = card.getByLabel('Full-summary prompt')
    const incPrompt = card.getByLabel('Incremental-summary prompt')

    await fullPrompt.click()
    await fullPrompt.press('ControlOrMeta+a')
    await fullPrompt.fill('Summarize this transcript: {transcript}')

    await incPrompt.click()
    await incPrompt.press('ControlOrMeta+a')
    await incPrompt.fill(
      'Update {previous_summary} with these new turns: {new_transcript}',
    )

    await card.locator('.ant-btn-primary[type="submit"]').click()
    await expect(page.getByText(SUCCESS_TOAST).first()).toBeVisible({
      timeout: 10000,
    })
  })

  // audit id 4c6db476f377 — the "Summarizer model" Select (the
  // default_summarization_model_id dropdown) was never opened or selected from
  // in E2E. Mock the chat-model list so the dropdown is deterministically
  // populated, open it, pick a model, and assert the PUT persists its id.
  test('summarizer model dropdown lists models and persists the selection', async ({
    page,
    testInfra,
  }) => {
    const MODEL_ID = '11111111-1111-1111-1111-111111111111'
    const MODEL_LABEL = 'Claude Test Model'

    await page.route(/\/api\/llm-models(\?.*)?$/, async route => {
      if (route.request().method() !== 'GET') return route.continue()
      return route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({
          models: [
            {
              id: MODEL_ID,
              name: 'claude-test',
              display_name: MODEL_LABEL,
              provider_id: '22222222-2222-2222-2222-222222222222',
            },
          ],
          page: 1,
          per_page: 200,
          total: 1,
        }),
      })
    })

    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    let putBody: any = null
    await page.route(/\/api\/summarization\/settings$/, async route => {
      const req = route.request()
      if (req.method() === 'PUT') {
        putBody = JSON.parse(req.postData() ?? '{}')
        return route.fulfill({
          status: 200,
          contentType: 'application/json',
          body: JSON.stringify({
            id: 1,
            enabled: true,
            default_summarization_model_id: putBody?.default_summarization_model_id,
            summarize_after_tokens: 20000,
            summarizer_keep_recent_tokens: 4000,
            updated_at: new Date().toISOString(),
          }),
        })
      }
      return route.continue()
    })

    // Reload so the mocked /api/llm-models populates the dropdown.
    await page.goto(`${testInfra.baseURL}/settings/summarization-admin`)
    const card = summarizationCard(page)
    await expect(card.getByLabel('Summarizer model')).toBeVisible({
      timeout: 30000,
    })

    // Open the antd Select and choose the mocked model.
    await card.getByLabel('Summarizer model').click()
    await page.getByRole('option', { name: MODEL_LABEL }).click()

    await card.locator('.ant-btn-primary[type="submit"]').click()
    await expect(page.getByText(SUCCESS_TOAST).first()).toBeVisible({
      timeout: 10000,
    })
    expect(putBody?.default_summarization_model_id).toBe(MODEL_ID)
  })
})
