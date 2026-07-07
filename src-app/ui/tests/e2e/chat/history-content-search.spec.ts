import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'

/**
 * ITEM-6 / TEST-11 — history search resolves SERVER-SIDE.
 *
 * Seeds enough conversations that a uniquely-titled one lands beyond the first
 * page (which the client loads on mount). Searching its term still surfaces it —
 * only possible if the query hits the backend, not the old client-only filter
 * over the loaded page. (Title-vs-message-content matching is proven against a
 * real DB by the backend integration tests.)
 */

async function seedConversation(
  apiURL: string,
  token: string,
  title: string,
): Promise<string> {
  const res = await fetch(`${apiURL}/api/conversations`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${token}` },
    body: JSON.stringify({ title }),
  })
  if (!res.ok) throw new Error(`seed failed: ${res.status} ${await res.text()}`)
  return (await res.json()).id as string
}

const UNIQUE = 'Xyzzy-marker topic'

test.describe('Chat history — server-side search', () => {
  test('finds a conversation on a later page (server-side, not client-filtered)', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)

    // Seed the unique one FIRST so it's the OLDEST → it sits on a later page
    // (default sort is recent). The 24 newer fillers fill page 1 (limit 20).
    const uniqueId = await seedConversation(apiURL, token, UNIQUE)
    for (let i = 0; i < 24; i++) {
      await seedConversation(apiURL, token, `Filler conversation ${i}`)
    }

    await page.goto(`${baseURL}/chats`)
    // Page 1 shows fillers; the unique (oldest) conversation is NOT loaded yet.
    await expect(page.getByTestId('chat-conversation-search-input')).toBeVisible({
      timeout: 30000,
    })
    await expect(page.getByTestId(`chat-conversation-card-${uniqueId}`)).toHaveCount(0)

    // Searching its term surfaces it — only possible via a server-side query.
    await page.getByTestId('chat-conversation-search-input').fill('Xyzzy')
    await expect(page.getByTestId(`chat-conversation-card-${uniqueId}`)).toBeVisible({
      timeout: 15000,
    })

    // A term matching nothing → empty state.
    await page.getByTestId('chat-conversation-search-input').fill('qqzz-nomatch')
    await expect(page.getByTestId('chat-history-empty')).toBeVisible({ timeout: 15000 })
  })
})
