import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'

/**
 * E2E — Summarization admin settings page renders the ERROR state (not a
 * blank card) when the settings GET fails.
 *
 * Covers SummarizationSettingsSection.tsx (`if (error && !settings)` →
 * `<Alert type="error" title="Failed to load summarization settings" />`)
 * and SummarizationAdmin.store.ts `load()` catch branch which stores the
 * error message. The only thing mocked is the external HTTP boundary
 * (the GET upstream); the store + component error path run for real.
 */

const SETTINGS_GET = '**/api/summarization/settings'

test.describe('Summarization — admin settings load error', () => {
  test('renders the error Alert when the settings GET returns 500', async ({
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

    await expect(
      page.getByText('Failed to load summarization settings'),
    ).toBeVisible({ timeout: 30000 })

    // The threshold form fields must NOT render in the error state — the
    // card short-circuits to the Alert before the <Form>.
    await expect(
      page.getByLabel('Summarize after N tokens'),
    ).toHaveCount(0)
  })
})
