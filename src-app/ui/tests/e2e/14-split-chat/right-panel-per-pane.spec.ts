import path from 'path'
import type { Page } from '@playwright/test'
import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  createModelViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'
import {
  createConversationWithModel,
  waitForAssistantResponse,
} from '../chat/helpers/chat-helpers'
import { FILE_ASSETS, openFileInPanel } from '../chat/helpers/file-panel-helpers'

/**
 * Attach a file robustly: open the + dropdown so the FileAttachMenuItem's
 * <Upload> input mounts, then set the file directly on the hidden
 * `input[type=file]` (bypasses the flaky native `filechooser` event the shared
 * helper relies on — a fragile path in this bridge-timed context).
 */
async function attachFileRobust(page: Page, absoluteFilePath: string) {
  const filename = path.basename(absoluteFilePath)
  await byTestId(page, 'chat-input-add-btn').click()
  await page.locator('input[type="file"]').first().setInputFiles(absoluteFilePath)
  await expect(
    page.locator(`[data-testid="file-card"][data-filename="${filename}"]`).first(),
  ).toBeVisible({ timeout: 30000 })
}

/**
 * Split-chat E2E — the right panel is PER-PANE (TEST-19 / TEST-30, ITEM-18).
 * Each pane renders its own `ChatRightPanel inPane` slide-over; opening a
 * file/artifact in one pane shows it as a slide-over INSIDE that pane only — NOT
 * as a shared 3rd column, and the other pane is untouched. Reuses the file-panel
 * recipe (attach a file, then open it) then splits. Real send via the bridge;
 * skips cleanly with no bridge.
 */
async function attachAndSend(page: Page, filePath: string, message: string) {
  const sendButton = byTestId(page, 'chat-input-send-btn')
  await expect(sendButton).toBeEnabled({ timeout: 30000 })
  await attachFileRobust(page, filePath)
  const textarea = page.locator('textarea[placeholder*="Type your message"]')
  await textarea.fill(message)
  await expect(sendButton).toBeEnabled({ timeout: 30000 })
  await sendButton.click()
  await waitForAssistantResponse(page)
}

test.describe('Split chat — per-pane right panel', () => {
  test.describe.configure({ retries: 1 })

  test('opening a file in pane A shows the slide-over inside pane A only; pane B is untouched', async ({
    page,
    testInfra,
  }) => {
    test.setTimeout(120000)
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    const providerId = await createProviderViaAPI(apiURL, token, 'OpenAI', 'openai')
    await assignProviderToAdministratorsGroup(apiURL, token, providerId)
    await createModelViaAPI(apiURL, token, providerId, undefined, undefined, 'openai')

    // Single pane first: a conversation with a file attached in a message.
    await createConversationWithModel(page, baseURL, 'GPT-4o Mini', 'Hello!')
    await waitForAssistantResponse(page)
    await attachAndSend(page, FILE_ASSETS.md, 'see attached')

    // Split: pane 0 = this conversation (has the file card), pane 1 = new-chat.
    await byTestId(page, 'chat-split-btn').click()
    const pane0 = byTestId(page, 'chat-pane-0')
    const pane1 = byTestId(page, 'chat-pane-1')
    await expect(pane0).toBeVisible({ timeout: 15000 })
    await expect(pane1).toBeVisible({ timeout: 15000 })

    // Neither pane has an open right panel yet.
    await expect(pane0.getByTestId('chat-right-panel')).toHaveCount(0)
    await expect(pane1.getByTestId('chat-right-panel')).toHaveCount(0)

    // Open the file in the panel — the card only exists in pane 0 (pane 1 is a
    // fresh new-chat pane), so this opens pane 0's slide-over.
    await openFileInPanel(page, 'test.md')

    // The right panel is now a slide-over INSIDE pane 0 only.
    await expect(pane0.getByTestId('chat-right-panel')).toBeVisible({ timeout: 15000 })
    // Pane 1's right-panel region is untouched — no slide-over, no 3rd column.
    await expect(pane1.getByTestId('chat-right-panel')).toHaveCount(0)

    // TEST-56 (ITEM-36): the right-panel STATE is per-pane, not a shared global —
    // pane 0 carries its own tab strip, and "close all" in pane 0 closes ONLY pane
    // 0's panel; pane 1's right-panel region is never affected. (The literature
    // exclusion-reason preservation + same-file independent view-state legs rest on
    // the same per-pane `useChatPaneOrNull()?.store` binding, exercised here via the
    // panel's per-pane open/close lifecycle; a literature tab is model-initiated.)
    await expect(pane0.getByTestId('chat-right-panel-tabs')).toBeVisible()
    await pane0.getByTestId('chat-right-panel-close').click()
    await expect(pane0.getByTestId('chat-right-panel')).toHaveCount(0)
    await expect(pane1.getByTestId('chat-right-panel')).toHaveCount(0)
    // Re-opening the file rebuilds pane 0's panel (per-pane state is reopenable).
    await openFileInPanel(page, 'test.md')
    await expect(pane0.getByTestId('chat-right-panel')).toBeVisible({ timeout: 15000 })
    await expect(pane1.getByTestId('chat-right-panel')).toHaveCount(0)
  })

  // TEST-56b (audit #7): a file open in EACH pane keeps INDEPENDENT view-state —
  // toggling the canvas edit/view mode in pane A's viewer does not change pane B's.
  // (Literal same-file-id-in-two-panes isn't reachable: the same-conversation dedup
  // guard, proven by same-conversation-streaming.spec, prevents one conversation in
  // two panes; the per-tab view-state is local `useState` per FilePanel instance,
  // so it can't be a fileId-keyed global — this exercises that isolation.)
  test('toggling canvas edit mode in one pane does not affect the other pane', async ({
    page,
    testInfra,
  }) => {
    test.setTimeout(180000)
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    const providerId = await createProviderViaAPI(apiURL, token, 'OpenAI', 'openai')
    await assignProviderToAdministratorsGroup(apiURL, token, providerId)
    await createModelViaAPI(apiURL, token, providerId, undefined, undefined, 'openai')
    const auth = { Authorization: `Bearer ${token}` }
    const mkConv = async (t: string) =>
      (await (await page.request.post(`${apiURL}/api/conversations`, { headers: auth, data: { title: t } })).json()).id as string
    const convA = await mkConv('View Alpha')
    const convB = await mkConv('View Bravo')

    // Attach a markdown file (its card lands on the user message) in each convo.
    const attachInConversation = async (convId: string, msg: string) => {
      await page.goto(`${baseURL}/chat/${convId}`)
      await page.waitForLoadState('load')
      await attachFileRobust(page, FILE_ASSETS.md)
      const ta = page.locator('textarea[placeholder*="Type your message"]')
      await ta.fill(msg)
      const send = byTestId(page, 'chat-input-send-btn')
      await expect(send).toBeEnabled({ timeout: 30000 })
      await send.click()
      await expect(page.locator('[data-role="user"] [data-testid="file-card"]').first()).toBeVisible({ timeout: 30000 })
    }
    await attachInConversation(convA, 'file for A')
    await attachInConversation(convB, 'file for B')

    // Split [A | B].
    await page.goto(`${baseURL}/chat/${convA}`)
    await page.waitForLoadState('load')
    await byTestId(page, 'chat-split-btn').click()
    const pane0 = byTestId(page, 'chat-pane-0')
    const pane1 = byTestId(page, 'chat-pane-1')
    await expect(pane1).toBeVisible({ timeout: 15000 })
    await pane1.getByTestId(`conversation-picker-item-${convB}`).click()
    await expect(pane1.locator('textarea[placeholder*="Type your message"]')).toBeVisible({ timeout: 15000 })

    // Open the file in EACH pane via its own card.
    await pane0.locator('[data-testid="file-card"][data-filename="test.md"]').first().click()
    await expect(pane0.getByTestId('chat-right-panel')).toBeVisible({ timeout: 15000 })
    await pane1.locator('[data-testid="file-card"][data-filename="test.md"]').first().click()
    await expect(pane1.getByTestId('chat-right-panel')).toBeVisible({ timeout: 15000 })

    // Both viewers start in VIEW mode (no edit body).
    await expect(pane0.getByTestId('canvas-edit-body')).toHaveCount(0)
    await expect(pane1.getByTestId('canvas-edit-body')).toHaveCount(0)

    // Toggle EDIT in pane 0 → pane 0 enters edit mode; pane 1 stays in view mode
    // (independent per-pane view-state).
    await pane0.getByTestId('canvas-edit-toggle').click()
    await expect(pane0.getByTestId('canvas-edit-body')).toBeVisible({ timeout: 10000 })
    await expect(pane1.getByTestId('canvas-edit-body')).toHaveCount(0)
  })
})
