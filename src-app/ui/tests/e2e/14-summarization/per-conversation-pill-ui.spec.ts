import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'

/**
 * E2E — the SummarizationStatusPill in the chat composer (the in-app UI for
 * the per-conversation summarization override).
 *
 * Audit gap: per-conversation-toggle.spec drives PUT /summarization-mode via
 * the API directly; the composer pill (SummarizationStatusPill.tsx) was never
 * exercised through the UI. This opens a conversation, clicks the pill, picks
 * "Never summarize this conversation", and asserts the PUT fires. The pill is
 * an antd Tag (aria-label addressed via getByLabel). No LLM needed.
 */

test.describe('Summarization — composer pill UI', () => {
  test('clicking the pill sets the per-conversation mode', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)

    const convRes = await fetch(`${apiURL}/api/conversations`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${token}` },
      body: JSON.stringify({ title: 'Pill conv' }),
    })
    const convId = (await convRes.json()).id as string

    await page.goto(`${baseURL}/chat/${convId}`)

    // The pill renders in the composer (aria-label starts "Summarization override:").
    const pill = page.getByLabel(/Summarization override:/).first()
    await expect(pill).toBeVisible({ timeout: 30000 })
    await pill.click()

    // Pick "Never summarize" → PUT /summarization-mode.
    const putResp = page.waitForResponse(
      r =>
        r.url().includes(`/conversations/${convId}/summarization-mode`) &&
        r.request().method() === 'PUT',
      { timeout: 30000 },
    )
    await page
      .getByRole('menuitem', { name: /Never summarize this conversation/ })
      .click()
    expect((await putResp).status()).toBeLessThan(400)
  })
})
