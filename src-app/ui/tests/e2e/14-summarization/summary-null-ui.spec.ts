import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'

// audit id all-2c8186fe48a3 — the null-summary case was only asserted via the
// API (GET /summary → null). Through the UI, SummaryBoundaryMarker bails on a
// null summary (SummaryBoundaryMarker.tsx:37 `if (!current?.summary) return
// null`), so a conversation with no summary must render WITHOUT a condensed-
// summary divider. We mock the summary endpoint to null and assert the chat
// renders with no summary marker.
test.describe('Summarization — null summary in the chat UI', () => {
  test('a conversation with no summary shows no summary boundary marker', async ({ page, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)

    const conv = await (
      await fetch(`${apiURL}/api/conversations`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${token}` },
        body: JSON.stringify({ title: 'no-summary conv' }),
      })
    ).json()
    const convId = conv.id as string

    // The summary endpoint returns null (the documented null case).
    await page.route(/\/api\/conversations\/[^/]+\/summary$/, async route =>
      route.fulfill({ status: 200, contentType: 'application/json', body: 'null' }),
    )

    await page.goto(`${baseURL}/chat/${convId}`)
    await page.waitForLoadState('domcontentloaded')

    // The chat composer renders (page is healthy with a null summary)...
    await expect(page.locator('textarea[placeholder*="Type your message"]').first()).toBeVisible({
      timeout: 30000,
    })
    // ...and NO condensed-conversation summary marker is shown.
    await expect(
      page.getByRole('button', { name: /condensed-conversation summary/i }),
    ).toHaveCount(0)
  })
})
