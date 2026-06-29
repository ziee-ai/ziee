import type { Page } from '@playwright/test'
import { byTestId } from '../testid'
import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'

/**
 * E2E for conversation deletion added in the hub cherry-pick:
 *   - per-card delete via a "Delete conversation?" Popconfirm (ConversationCard)
 *   - bulk select + "Delete Selected" via the selection bar (ConversationList)
 *
 * Conversations are seeded directly via POST /api/conversations (title only,
 * no model needed) so the tests stay fast and deterministic.
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

/** Locate the conversation Card by its conversation id (kit derived testid). */
function cardById(page: Page, id: string) {
  return byTestId(page, `chat-conversation-card-${id}`)
}

async function gotoChats(page: Page, baseURL: string) {
  await page.goto(`${baseURL}/chats`)
  // NOTE: do NOT wait for 'networkidle' here. The chat realtime-sync feature
  // opens two always-on SSE streams (chat tokens + sync) the moment a chat page
  // mounts; those long-lived requests never "finish", so Playwright's
  // networkidle never fires and the helper hangs until the test times out.
  // Wait for a concrete render signal instead — the seeded cards are asserted
  // visible by each caller right after.
  await page.waitForLoadState('domcontentloaded')
}

test.describe('Conversation deletion', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    await loginAsAdmin(page, testInfra.baseURL)
  })

  test('delete a single conversation via the card Popconfirm', async ({ page, testInfra }) => {
    const token = await getAdminToken(testInfra.apiURL)
    const delId = await seedConversation(testInfra.apiURL, token, 'E2E Delete Me')
    const keepId = await seedConversation(testInfra.apiURL, token, 'E2E Keep Me')

    await gotoChats(page, testInfra.baseURL)

    const target = cardById(page, delId)
    await expect(target).toBeVisible({ timeout: 15000 })

    // Reveal + click the per-card delete (icon-only) button, then confirm.
    await target.hover()
    await byTestId(page, `chat-conversation-delete-btn-${delId}`).click()

    const confirm = byTestId(page, `chat-conversation-delete-confirm-${delId}`)
    await expect(confirm).toBeVisible()
    await byTestId(page, `chat-conversation-delete-confirm-${delId}-confirm`).click()

    await expect(cardById(page, delId)).toHaveCount(0, { timeout: 10000 })
    // The other conversation is untouched.
    await expect(cardById(page, keepId)).toBeVisible()
  })

  test('bulk-delete selected conversations', async ({ page, testInfra }) => {
    const token = await getAdminToken(testInfra.apiURL)
    const oneId = await seedConversation(testInfra.apiURL, token, 'E2E Bulk One')
    const twoId = await seedConversation(testInfra.apiURL, token, 'E2E Bulk Two')

    await gotoChats(page, testInfra.baseURL)

    const one = cardById(page, oneId)
    const two = cardById(page, twoId)
    await expect(one).toBeVisible({ timeout: 15000 })
    await expect(two).toBeVisible()

    // Select both via their checkboxes; the first selection enters selection mode.
    await one.hover()
    await byTestId(page, `chat-conversation-select-${oneId}`).click()
    await two.hover()
    await byTestId(page, `chat-conversation-select-${twoId}`).click()

    // The bulk-action bar appears once something is selected (count is live data).
    const bulkBar = byTestId(page, 'chat-bulk-actions-card')
    await expect(bulkBar).toBeVisible()
    await expect(bulkBar).toContainText('2 conversation')

    await byTestId(page, 'chat-bulk-delete-btn').click()
    const confirm = byTestId(page, 'chat-bulk-delete-confirm')
    await expect(confirm).toBeVisible()
    await byTestId(page, 'chat-bulk-delete-confirm-confirm').click()

    await expect(cardById(page, oneId)).toHaveCount(0, { timeout: 10000 })
    await expect(cardById(page, twoId)).toHaveCount(0)
  })
})
