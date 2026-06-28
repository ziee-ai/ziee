import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'

/**
 * E2E — the MemoryStatusPill in the chat composer (the per-conversation memory
 * override pill).
 *
 * Audit gap: 12-memory/per-conversation-toggle.spec exercises the memory-mode
 * API; the composer pill (MemoryStatusPill.tsx) was never driven through the
 * UI. This opens a conversation, clicks the pill, picks a different mode, and
 * asserts the per-conversation memory-mode PUT fires. No LLM needed.
 */

test.describe('Memory — composer pill UI', () => {
  test('clicking the memory pill sets the per-conversation mode', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)

    const convRes = await fetch(`${apiURL}/api/conversations`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${token}` },
      body: JSON.stringify({ title: 'Mem pill conv' }),
    })
    const convId = (await convRes.json()).id as string

    await page.goto(`${baseURL}/chat/${convId}`)

    const pill = page.getByLabel(/Memory mode:/).first()
    await expect(pill).toBeVisible({ timeout: 30000 })
    await pill.click()

    // The dropdown exposes the memory-mode options; selecting one PUTs the
    // per-conversation memory mode.
    const putResp = page.waitForResponse(
      r =>
        r.url().includes(`/conversations/${convId}/memory-mode`) &&
        r.request().method() === 'PUT',
      { timeout: 30000 },
    )
    await page.getByRole('menuitem', { name: /Always retrieve memories/ }).click()
    expect((await putResp).status()).toBeLessThan(400)
  })
})
