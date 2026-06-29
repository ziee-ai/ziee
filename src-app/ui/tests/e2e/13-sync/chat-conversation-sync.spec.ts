import { test, expect } from '../../fixtures/test-context'
import {
  loginAsAdmin,
  login,
  createTestUser,
  getAdminToken,
} from '../../common/auth-helpers'
import { goToNewChatPage } from '../09-chat/helpers/chat-helpers'

// Cross-USER isolation for CONVERSATION realtime sync.
//
// `conversation-sync.spec.ts` covers the same-user cross-window delivery
// (create / rename / delete propagate to the owner's other window), and
// `chat-stream-sync.spec.ts` covers cross-device live token streaming — but
// neither asserts the negative control that EVERY other owner-scoped sync
// entity in this suite does (memory-sync: "reaches the owner's other device
// but NOT a different user"; assistant-sync; realtime-sync/project). The
// `conversation` entity is published with `Audience::owner(user_id)`, so a
// different user's device must NEVER receive another user's conversation
// over the SSE fan-out. That isolation was untested for conversations.
//
// Run with --workers=1 (shared backend + DB). Deterministic — the mutation is
// a REST API create, no LLM needed; the assertion is that the owner's OTHER
// device lists it live (positive control, which also establishes the delivery
// window has elapsed) while a separate user's device never does.

async function createConversation(
  baseURL: string,
  token: string,
  title: string,
): Promise<string> {
  const res = await fetch(`${baseURL}/api/conversations`, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      Authorization: `Bearer ${token}`,
    },
    body: JSON.stringify({ title }),
  })
  if (!res.ok) {
    throw new Error(
      `create conversation failed: ${res.status} ${await res.text()}`,
    )
  }
  return (await res.json()).id as string
}

test.describe('Realtime sync — conversation (cross-user isolation)', () => {
  test("a conversation reaches the owner's other device but NOT a different user", async ({
    page,
    browser,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra

    // Admin first: loginAsAdmin onboards the fresh per-test backend so
    // getAdminToken / createTestUser work afterward. The admin is the
    // conversation OWNER (devices A + A2); a separate user is the probe.
    await loginAsAdmin(page, baseURL)
    await goToNewChatPage(page, baseURL)

    const adminToken = await getAdminToken(apiURL)
    const uniq = Date.now()
    const username = `conv_other_${uniq}`
    const password = 'password123'
    await createTestUser(
      apiURL,
      adminToken,
      username,
      `${username}@ex.com`,
      password,
      // Read-only is enough to load the chat page + recent-conversations
      // sidebar; the Chat store self-gates its sync:conversation refetch on
      // conversations::read (the no-403 reconnect rule).
      ['profile::read', 'profile::edit', 'conversations::read'],
    )

    const ctxA2 = await browser.newContext() // owner, device 2 — positive control
    const pageA2 = await ctxA2.newPage()
    const ctxB = await browser.newContext() // different user — isolation probe
    const pageB = await ctxB.newPage()
    try {
      await loginAsAdmin(pageA2, baseURL)
      await goToNewChatPage(pageA2, baseURL)
      await login(pageB, baseURL, username, password)
      await goToNewChatPage(pageB, baseURL)

      const title = `XSync Isolation Conv ${uniq}`
      await createConversation(baseURL, adminToken, title)

      // Positive control: the owner's OTHER device lists it live (no reload),
      // which also proves the SSE delivery window has elapsed.
      await expect(pageA2.getByTestId(/^chat-recent-conversations-menu-item-/).filter({ hasText: title })).toBeVisible({
        timeout: 15_000,
      })

      // Isolation: the different user's device — within the same delivery
      // window — never sees the admin's conversation (owner-scoped audience).
      await expect(pageB.getByTestId(/^chat-recent-conversations-menu-item-/).filter({ hasText: title })).not.toBeVisible()
    } finally {
      await ctxA2.close()
      await ctxB.close()
    }
  })
})
