import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { byTestId } from '../testid'

/**
 * E2E — Summarization admin settings page (migration 91 extraction).
 *
 * Retargeted from the deleted `12-memory/summarizer-thresholds.spec.ts`
 * to the new dedicated `/settings/summarization-admin` page.
 *
 *   - validation path: trigger 5000, keep 6000 → inline error
 *     "Keep-recent (6000) must be less than the trigger (5000)." + NO
 *     success toast (the form's pre-submit validator never lets the PUT
 *     fire).
 *   - happy path: trigger 20000, keep 4000 → "Summarization settings
 *     saved." toast.
 */

const SUCCESS_TOAST = 'Summarization settings saved.'

/** Sonner toast lanes (i18n-safe: scoped by data-type, asserted on data text). */
function successToast(page: import('@playwright/test').Page) {
  return page.locator('[data-sonner-toast][data-type="success"]')
}
function errorToast(page: import('@playwright/test').Page) {
  return page.locator('[data-sonner-toast][data-type="error"]')
}

/** The summarization settings card scope. */
function summarizationCard(page: import('@playwright/test').Page) {
  return byTestId(page, 'summ-settings-card')
}

/** Set a kit InputNumber (addressed by testid) to `value`. */
async function setNumberField(
  field: import('@playwright/test').Locator,
  value: number,
) {
  await field.click()
  await field.press('ControlOrMeta+a')
  await field.fill(String(value))
  // Commit the value (formats on blur / Enter).
  await field.press('Enter')
}

test.describe('Summarization — admin thresholds', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    await loginAsAdmin(page, testInfra.baseURL)
    await page.goto(`${testInfra.baseURL}/settings/summarization-admin`)
    await expect(summarizationCard(page)).toBeVisible({ timeout: 30000 })
  })

  test('rejects keep >= trigger with an inline error and no success toast', async ({
    page,
  }) => {
    const card = summarizationCard(page)
    const trigger = byTestId(card, 'summ-after-tokens-input')
    const keep = byTestId(card, 'summ-keep-recent-input')

    // keep (6000) >= trigger (5000) — invalid.
    await setNumberField(trigger, 5000)
    await setNumberField(keep, 6000)

    await byTestId(card, 'summ-save-button').click()

    // The dynamic validation message reflects the exact values the test typed.
    await expect(errorToast(page)).toContainText(
      'Keep-recent (6000) must be less than the trigger (5000).',
      { timeout: 10000 },
    )

    // The form never submitted → no success toast.
    await expect(successToast(page)).toHaveCount(0)
  })

  test('saves a valid trigger/keep pair', async ({ page }) => {
    const card = summarizationCard(page)
    const trigger = byTestId(card, 'summ-after-tokens-input')
    const keep = byTestId(card, 'summ-keep-recent-input')

    // keep (4000) < trigger (20000) — valid.
    await setNumberField(trigger, 20000)
    await setNumberField(keep, 4000)

    await byTestId(card, 'summ-save-button').click()

    await expect(successToast(page)).toContainText(SUCCESS_TOAST, {
      timeout: 10000,
    })
  })

  test('rejects full_summary_prompt missing the {transcript} placeholder', async ({
    page,
  }) => {
    const card = summarizationCard(page)
    const fullPrompt = byTestId(card, 'summ-full-prompt-textarea')
    // Non-empty value that lacks `{transcript}` — the pre-submit validator
    // must reject so the engine never gets a template it can't interpolate.
    await fullPrompt.click()
    await fullPrompt.press('ControlOrMeta+a')
    await fullPrompt.fill('Summarize this conversation, please.')
    await byTestId(card, 'summ-save-button').click()

    // Inline error from the form's pre-submit validator — match on the
    // placeholder name in the error toast.
    await expect(errorToast(page)).toContainText(/\{transcript\}/i, {
      timeout: 10000,
    })
    await expect(successToast(page)).toHaveCount(0)
  })

  test('toggling the "Enable summarization" Switch persists across reload', async ({
    page,
    testInfra,
  }) => {
    const card = summarizationCard(page)
    const toggle = byTestId(card, 'summ-enabled-switch')
    await expect(toggle).toBeVisible({ timeout: 10000 })

    const before = await toggle.getAttribute('aria-checked')
    await toggle.click()
    const after = await toggle.getAttribute('aria-checked')
    expect(after).not.toBe(before)

    await byTestId(card, 'summ-save-button').click()
    await expect(successToast(page)).toContainText(SUCCESS_TOAST, {
      timeout: 10000,
    })

    // Reload — the persisted value must come back (the PUT actually fired).
    await page.goto(`${testInfra.baseURL}/settings/summarization-admin`)
    const reloaded = byTestId(summarizationCard(page), 'summ-enabled-switch')
    await expect(reloaded).toHaveAttribute('aria-checked', after!, {
      timeout: 10000,
    })

    // Restore the original state so re-runs start from a known baseline.
    await reloaded.click()
    await byTestId(summarizationCard(page), 'summ-save-button').click()
    await expect(successToast(page)).toContainText(SUCCESS_TOAST, {
      timeout: 10000,
    })
  })

  test('saves valid prompt overrides', async ({ page }) => {
    const card = summarizationCard(page)
    const fullPrompt = byTestId(card, 'summ-full-prompt-textarea')
    const incPrompt = byTestId(card, 'summ-incremental-prompt-textarea')

    await fullPrompt.click()
    await fullPrompt.press('ControlOrMeta+a')
    await fullPrompt.fill('Summarize this transcript: {transcript}')

    await incPrompt.click()
    await incPrompt.press('ControlOrMeta+a')
    await incPrompt.fill(
      'Update {previous_summary} with these new turns: {new_transcript}',
    )

    await byTestId(card, 'summ-save-button').click()
    await expect(successToast(page)).toContainText(SUCCESS_TOAST, {
      timeout: 10000,
    })
  })

  // audit id 4c6db476f377 — the "Summarizer model" Combobox (the
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
    await expect(byTestId(card, 'summ-model-combobox')).toBeVisible({
      timeout: 30000,
    })

    // Open the model Combobox and choose the mocked model (derived option id).
    await byTestId(card, 'summ-model-combobox').click()
    await byTestId(page, `summ-model-combobox-opt-${MODEL_ID}`).click()

    await byTestId(card, 'summ-save-button').click()
    await expect(successToast(page)).toContainText(SUCCESS_TOAST, {
      timeout: 10000,
    })
    expect(putBody?.default_summarization_model_id).toBe(MODEL_ID)
  })
})
