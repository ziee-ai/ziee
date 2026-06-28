import { test, expect } from './no-403'
import { loginAsMember } from './fixtures'

/**
 * Summarization admin page is gated by `SummarizationSettingsRead`. The
 * existing 14-summarization specs always log in as admin, so the GATE itself
 * (a non-admin being kept out) was never exercised. A basic member must not see
 * the "Summarization" settings entry and must hit the inline 403 on a deep-link.
 * Runs under the no-403 fixture, so any accidental admin-only API call also
 * fails the test.
 */
test.describe('summarization module — permission gating', () => {
  test('non-admin: entry hidden + deep-link renders 403', async ({
    page,
    testInfra,
  }) => {
    await loginAsMember(page, testInfra.baseURL, testInfra.apiURL)

    // The "Summarization" admin entry is absent from a non-admin's settings menu.
    await page.goto(`${testInfra.baseURL}/settings`)
    await expect(
      page.getByRole('menuitem', { name: /^Summarization$/ }),
    ).toHaveCount(0)

    // Deep-link to the admin page → inline 403 (URL preserved).
    await page.goto(`${testInfra.baseURL}/settings/summarization-admin`)
    await expect(page.getByText(/Not authorized/i)).toBeVisible()
    expect(page.url()).toContain('/settings/summarization-admin')
  })
})
