import type { Page } from '@playwright/test'
import { byTestId } from '../testid'
import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import { goToNewChatPage } from '../chat/helpers/chat-helpers'
import {
  createProviderViaAPI,
  createModelViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'

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
      await expect(page.getByTestId(/^chat-recent-conversations-menu-item-/).filter({ hasText: title })).toBeVisible({
        timeout: 15_000,
      })
      await expect(pageB.getByTestId(/^chat-recent-conversations-menu-item-/).filter({ hasText: title })).toBeVisible({
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
      await expect(pageB.getByTestId(/^chat-recent-conversations-menu-item-/).filter({ hasText: original })).toBeVisible({
        timeout: 15_000,
      })

      // Rename → device B shows the new title.
      const renamed = `${original} (renamed)`
      await renameConversation(baseURL, token, id, renamed)
      await expect(pageB.getByTestId(/^chat-recent-conversations-menu-item-/).filter({ hasText: renamed })).toBeVisible({
        timeout: 15_000,
      })

      // Delete → device B drops it from the list.
      await deleteConversation(baseURL, token, id)
      await expect(pageB.getByTestId(/^chat-recent-conversations-menu-item-/).filter({ hasText: renamed })).toHaveCount(0, { timeout: 15_000 })
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
      await expect(pageB.getByTestId(/^chat-recent-conversations-menu-item-/).filter({ hasText: title })).toHaveCount(0, { timeout: 15_000 })
    } finally {
      await ctxB.close()
    }
  })
})

// ── Sidebar sync driven by the REAL chat INPUT (not an API mutation) ──────────
//
// The tests above issue mutations over REST. This block closes the gap where
// the conversation is born from the UI itself: a user types a first message on
// the New-chat page (device A), which creates the conversation server-side and
// streams a reply. Device B's recent-conversations sidebar must list the new
// conversation live (the `sync:conversation` Create notify→refetch path), WITHOUT
// device B reloading. Needs a real model, so soft-skipped without ANTHROPIC_API_KEY.

const ANTHROPIC_KEY = process.env.ANTHROPIC_API_KEY ?? ''

test.describe('Realtime sync — conversation born from chat input (cross-window)', () => {
  test.skip(
    ANTHROPIC_KEY.length === 0,
    'ANTHROPIC_API_KEY not set — real-LLM new-chat sidebar sync skipped',
  )

  test('sending a first message on device A surfaces the conversation in device B sidebar', async ({
    page,
    browser,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra

    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)
    const providerId = await createProviderViaAPI(
      apiURL,
      adminToken,
      'Anthropic',
      'anthropic',
    )
    await assignProviderToAdministratorsGroup(apiURL, adminToken, providerId)
    await createModelViaAPI(
      apiURL,
      adminToken,
      providerId,
      'claude-haiku-4-5-20251001',
      'Claude Haiku 4.5',
      'anthropic',
    )

    const ctxB = await browser.newContext()
    const pageB = await ctxB.newPage()
    try {
      await loginAsAdmin(pageB, baseURL)
      await goToNewChatPage(pageB, baseURL)

      // Device A: the conversation does not exist yet — it is created by the
      // act of sending the first message from the New-chat page.
      await goToNewChatPage(page, baseURL)
      const marker = `XSync FromInput ${Date.now()}`
      const textarea = page.locator('textarea[placeholder*="Type your message"]')
      await textarea.fill(`Please acknowledge the phrase ${marker}.`)
      const sendButton = byTestId(page, 'chat-input-send-btn')
      await expect(sendButton).toBeEnabled({ timeout: 15_000 })
      await sendButton.click()

      // A navigates to the freshly created conversation.
      await page.waitForURL(/\/chat\/[a-f0-9-]+/, { timeout: 30_000 })

      // Device B's sidebar lists the new conversation live (notify→refetch),
      // matched by the auto-derived title (first user message text).
      await expect(pageB.getByTestId(/^chat-recent-conversations-menu-item-/).filter({ hasText: marker })).toBeVisible({
        timeout: 30_000,
      })
    } finally {
      await ctxB.close()
    }
  })
})

// ── UI-driven delete (Popconfirm) propagates the removal cross-window ─────────
//
// The earlier "rename and delete" test deletes over REST. This closes the gap
// where the DELETE is issued through the real UI affordance — the per-card
// "Delete conversation?" Popconfirm on the /chats list — and asserts BOTH that
// device A's card disappears AND device B's sidebar drops it via sync. No LLM:
// the conversation is API-seeded (title only), only the delete goes through UI.

function cardByTitle(p: Page, title: string) {
  return p.getByTestId(/^chat-conversation-card-/).filter({ hasText: title })
}

test.describe('Realtime sync — UI delete with confirmation (cross-window)', () => {
  test('deleting via the card Popconfirm on device A drops it from device B sidebar', async ({
    page,
    browser,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)

    const title = `XSync UIDelete ${Date.now()}`
    const res = await fetch(`${apiURL}/api/conversations`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        Authorization: `Bearer ${token}`,
      },
      body: JSON.stringify({ title }),
    })
    expect(res.ok).toBeTruthy()

    const ctxB = await browser.newContext()
    const pageB = await ctxB.newPage()
    try {
      await loginAsAdmin(pageB, baseURL)
      await goToNewChatPage(pageB, baseURL)
      await expect(pageB.getByTestId(/^chat-recent-conversations-menu-item-/).filter({ hasText: title })).toBeVisible({
        timeout: 15_000,
      })

      // Device A opens the chat list and deletes via the Popconfirm.
      await page.goto(`${baseURL}/chats`)
      await page.waitForLoadState('domcontentloaded')
      const target = cardByTitle(page, title)
      await expect(target).toBeVisible({ timeout: 15_000 })
      await target.hover()
      await target.getByTestId(/^chat-conversation-delete-btn-/).click()
      const confirmBtn = page.getByTestId(/^chat-conversation-delete-confirm-.+-confirm$/)
      await expect(confirmBtn).toBeVisible()
      await confirmBtn.click()

      // Device A's card is gone…
      await expect(cardByTitle(page, title)).toHaveCount(0, { timeout: 10_000 })
      // …and device B's sidebar drops it live via sync (no reload).
      await expect(pageB.getByTestId(/^chat-recent-conversations-menu-item-/).filter({ hasText: title })).toHaveCount(0, { timeout: 15_000 })
    } finally {
      await ctxB.close()
    }
  })
})
