import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import { goToNewChatPage } from '../09-chat/helpers/chat-helpers'

// Cross-window realtime sync for CONVERSATIONS (the chat *list* dimension).
//
// Run with --workers=1 (shared backend + DB). Two browser contexts, same
// admin user. Mutations are issued over the authenticated REST API so the test
// is deterministic and needs no LLM; the assertion is that the OTHER window's
// UI updates WITHOUT a reload — i.e. the `sync:conversation` notify→refetch path
// works end-to-end.
//
// NOTE: the live-TOKEN streaming dimension (both windows render a reply as it
// types in real time) is exercised deterministically at the backend layer in
// `server/tests/chat/chat_stream_test.rs` (stub-engine-backed). It is NOT
// covered here because Playwright's one-shot `route.fulfill` cannot model the
// pushed per-user chat-token stream; a faithful UI version needs either the
// stub-engine wired into global setup (a `custom` provider) or a CDP-level
// pushable SSE mock. See `.plans/feat-realtime-sync-chat-test-coverage.md`.

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

async function renameConversation(
  baseURL: string,
  token: string,
  id: string,
  title: string,
): Promise<void> {
  const res = await fetch(`${baseURL}/api/conversations/${id}`, {
    method: 'PUT',
    headers: {
      'Content-Type': 'application/json',
      Authorization: `Bearer ${token}`,
    },
    body: JSON.stringify({ title }),
  })
  if (!res.ok) {
    throw new Error(
      `rename conversation failed: ${res.status} ${await res.text()}`,
    )
  }
}

async function deleteConversation(
  baseURL: string,
  token: string,
  id: string,
): Promise<void> {
  const res = await fetch(`${baseURL}/api/conversations/${id}`, {
    method: 'DELETE',
    headers: { Authorization: `Bearer ${token}` },
  })
  if (!res.ok && res.status !== 204) {
    throw new Error(
      `delete conversation failed: ${res.status} ${await res.text()}`,
    )
  }
}

test.describe('Realtime sync (conversations, cross-window)', () => {
  test('a conversation created on device A appears in device B sidebar without reload', async ({
    page,
    browser,
    testInfra,
  }) => {
    const { baseURL } = testInfra

    // Device A — establishes the admin session so getAdminToken can auth.
    await loginAsAdmin(page, baseURL)
    await goToNewChatPage(page, baseURL)
    const token = await getAdminToken(baseURL)

    // Device B — second context for the SAME admin user.
    const ctxB = await browser.newContext()
    const pageB = await ctxB.newPage()
    try {
      await loginAsAdmin(pageB, baseURL)
      await goToNewChatPage(pageB, baseURL)

      const title = `XSync Conv ${Date.now()}`
      await createConversation(baseURL, token, title)

      // Both windows' recent-conversations sidebar must list it live.
      await expect(page.getByText(title).first()).toBeVisible({
        timeout: 15_000,
      })
      await expect(pageB.getByText(title).first()).toBeVisible({
        timeout: 15_000,
      })
    } finally {
      await ctxB.close()
    }
  })

  test('rename and delete on device A propagate to device B', async ({
    page,
    browser,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await goToNewChatPage(page, baseURL)
    const token = await getAdminToken(baseURL)

    const ctxB = await browser.newContext()
    const pageB = await ctxB.newPage()
    try {
      await loginAsAdmin(pageB, baseURL)
      await goToNewChatPage(pageB, baseURL)

      const original = `XSync Rename ${Date.now()}`
      const id = await createConversation(baseURL, token, original)
      await expect(pageB.getByText(original).first()).toBeVisible({
        timeout: 15_000,
      })

      // Rename → device B shows the new title.
      const renamed = `${original} (renamed)`
      await renameConversation(baseURL, token, id, renamed)
      await expect(pageB.getByText(renamed).first()).toBeVisible({
        timeout: 15_000,
      })

      // Delete → device B drops it from the list.
      await deleteConversation(baseURL, token, id)
      await expect(pageB.getByText(renamed)).toHaveCount(0, { timeout: 15_000 })
    } finally {
      await ctxB.close()
    }
  })

  test('deleting the conversation open on device B resets B and drops it from the list', async ({
    page,
    browser,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await goToNewChatPage(page, baseURL)
    const token = await getAdminToken(baseURL)

    const title = `XSync OpenDelete ${Date.now()}`
    const id = await createConversation(baseURL, token, title)

    const ctxB = await browser.newContext()
    const pageB = await ctxB.newPage()
    try {
      await loginAsAdmin(pageB, baseURL)
      // Device B has THIS conversation open. Navigate directly and wait for the
      // composer — an empty (API-created, no messages) conversation renders the
      // empty state without the `chat-messages` list container.
      await pageB.goto(`${baseURL}/chat/${id}`)
      await pageB.waitForSelector('textarea[placeholder*="Type your message"]', {
        timeout: 30_000,
      })

      // Device A deletes it out from under B.
      await deleteConversation(baseURL, token, id)

      // B's view must not keep pointing at the dead conversation: the sidebar
      // drops it (the Chat store also `reset()`s the open view on a remote
      // delete — see Chat.store `sync:conversation` handler).
      await expect(pageB.getByText(title)).toHaveCount(0, { timeout: 15_000 })
    } finally {
      await ctxB.close()
    }
  })
})
