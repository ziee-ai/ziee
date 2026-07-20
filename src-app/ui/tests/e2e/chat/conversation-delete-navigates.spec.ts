import type { Page } from '@playwright/test'
import { byTestId } from '../testid'
import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'

/**
 * E2E for issue #168 — "Interface after delete a chat".
 *
 * Deleting the conversation you are CURRENTLY VIEWING removed its sidebar row but
 * left the main content pane rendering the deleted conversation, with the URL stuck
 * on the dead id. Nothing owned that transition: the store deliberately doesn't
 * route, no call site navigates, `SplitView.closePaneForConversation` is a no-op in
 * single-pane mode (`panes` is empty), and `ConversationPage`'s workspace→URL effect
 * bails at `panes.length === 0`. `useNavigateAwayOnDelete` closes that gap.
 *
 * Drives the real backend (no `page.route` mocking); conversations are seeded via
 * POST /api/conversations (title only, no model needed) so no LLM is involved.
 */

async function seedConversation(
  apiURL: string,
  token: string,
  title: string,
): Promise<string> {
  const res = await fetch(`${apiURL}/api/conversations`, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      Authorization: `Bearer ${token}`,
    },
    body: JSON.stringify({ title }),
  })
  if (!res.ok) {
    throw new Error(
      `seedConversation(${title}) failed: ${res.status} ${await res.text()}`,
    )
  }
  return (await res.json()).id as string
}

/** Delete a conversation through the sidebar ⋯ menu (the LOCAL delete path). */
async function deleteFromSidebar(page: Page, id: string) {
  await byTestId(page, `chat-recent-row-actions-btn-${id}`).click({ force: true })
  await byTestId(page, `chat-recent-row-menu-${id}-item-delete`).click()
  await byTestId(page, 'chat-conversation-delete-confirm-btn').click()
}

/**
 * Open a conversation. NOTE: do NOT wait for 'networkidle' — the chat page opens
 * two always-on SSE streams (chat tokens + sync) that never finish, so networkidle
 * never fires. Wait for a concrete render signal instead.
 */
async function openConversation(page: Page, baseURL: string, id: string) {
  await page.goto(`${baseURL}/chat/${id}`)
  await page.waitForLoadState('domcontentloaded')
  await expect(byTestId(page, 'conversation-title')).toBeVisible({ timeout: 15000 })
}

test.describe('Deleting the open conversation navigates away (#168)', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    await loginAsAdmin(page, testInfra.baseURL)
  })

  test('deleting the currently-open conversation returns to the start page', async ({
    page,
    testInfra,
  }) => {
    const token = await getAdminToken(testInfra.apiURL)
    const openId = await seedConversation(testInfra.apiURL, token, 'E2E Nav Open')
    await seedConversation(testInfra.apiURL, token, 'E2E Nav Other')

    await openConversation(page, testInfra.baseURL, openId)
    await expect(byTestId(page, 'conversation-title')).toContainText('Nav Open')

    await deleteFromSidebar(page, openId)

    // The URL leaves the dead id for the start / new-chat page...
    await expect(page).toHaveURL(/\/chat$/, { timeout: 15000 })
    // ...and the main pane shows the new-chat greeting, not the deleted messages.
    await expect(byTestId(page, 'new-chat-greeting')).toBeVisible({ timeout: 15000 })
    await expect(byTestId(page, 'conversation-title')).toHaveCount(0)
  })

  test('deleting a NON-open conversation leaves the active pane untouched', async ({
    page,
    testInfra,
  }) => {
    const token = await getAdminToken(testInfra.apiURL)
    const openId = await seedConversation(testInfra.apiURL, token, 'E2E Stay Open')
    const otherId = await seedConversation(testInfra.apiURL, token, 'E2E Delete Other')

    await openConversation(page, testInfra.baseURL, openId)

    await deleteFromSidebar(page, otherId)

    // Its sidebar row goes...
    await expect(byTestId(page, `chat-recent-row-actions-btn-${otherId}`)).toHaveCount(
      0,
      { timeout: 15000 },
    )
    // ...but we must NOT over-navigate: the open conversation stays put.
    await expect(page).toHaveURL(new RegExp(`/chat/${openId}$`))
    await expect(byTestId(page, 'conversation-title')).toContainText('Stay Open')
  })

  test('deleting the last conversation lands on the start page with the empty state', async ({
    page,
    testInfra,
  }) => {
    const token = await getAdminToken(testInfra.apiURL)
    const onlyId = await seedConversation(testInfra.apiURL, token, 'E2E Last One')

    await openConversation(page, testInfra.baseURL, onlyId)

    await deleteFromSidebar(page, onlyId)

    await expect(page).toHaveURL(/\/chat$/, { timeout: 15000 })
    await expect(byTestId(page, 'new-chat-greeting')).toBeVisible({ timeout: 15000 })
    // The sidebar falls back to its "No conversations yet" empty state.
    await expect(byTestId(page, 'chat-recent-empty')).toBeVisible({ timeout: 15000 })
  })
})
