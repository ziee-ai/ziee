import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { byTestId } from '../testid'

/**
 * E2E — Summarization admin settings page renders the ERROR state (not a
 * blank card) when the settings GET fails.
 *
 * Covers SummarizationSettingsSection.tsx (`if (error && !settings)` →
 * `<ErrorState resource="summarization settings" onRetry={…} />`)
 * and SummarizationAdmin.store.ts `load()` catch branch which stores the
 * error message. The only thing mocked is the external HTTP boundary
 * (the GET upstream); the store + component error path run for real.
 */

const SETTINGS_GET = '**/api/summarization/settings'

test.describe('Summarization — admin settings load error', () => {
  test('renders the error state when the settings GET returns 500', async ({
    page,
    testInfra,
  }) => {
    await loginAsAdmin(page, testInfra.baseURL)

    // Fail ONLY the GET; let PUT and everything else pass through.
    await page.route(SETTINGS_GET, async route => {
      if (route.request().method() === 'GET') {
        await route.fulfill({
          status: 500,
          contentType: 'application/json',
          body: JSON.stringify({ error: 'boom' }),
        })
      } else {
        await route.fallback()
      }
    })

    await page.goto(`${testInfra.baseURL}/settings/summarization-admin`)

    await expect(byTestId(page, 'summ-settings-error-card')).toBeVisible({
      timeout: 30000,
    })
    await expect(byTestId(page, 'summ-settings-error')).toBeVisible()

    // The threshold form fields must NOT render in the error state — the
    // card short-circuits to the ErrorState before the <Form>.
    await expect(byTestId(page, 'summ-after-tokens-input')).toHaveCount(0)
  })
})
