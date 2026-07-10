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
const HAS_BRIDGE = Boolean(
  process.env.OPENAI_BASE_URL || process.env.ZIEE_TEST_LLM_BASE_URL,
)

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
  test.skip(!HAS_BRIDGE, 'OPENAI_BASE_URL/ZIEE_TEST_LLM_BASE_URL not set — skipping real-send E2E')
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
  })
})
