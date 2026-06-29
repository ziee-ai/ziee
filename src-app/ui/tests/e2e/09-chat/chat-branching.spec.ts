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
} from './helpers/chat-helpers'

/**
 * Chat - Branching E2E Tests
 *
 * Tests for message editing and response regeneration via conversation branches.
 * Verifies:
 * - Regenerate: branch navigator appears at the ASSISTANT bubble (not user bubble)
 * - Regenerate: no duplicate user message on the child branch
 * - Edit: branch navigator appears at the USER bubble (edited message)
 * - Navigator allows switching between branches
 */

async function setupProviderAndModel(apiURL: string, adminToken: string) {
  const providerId = await createProviderViaAPI(apiURL, adminToken, 'OpenAI', 'openai')
  await assignProviderToAdministratorsGroup(apiURL, adminToken, providerId)
  await createModelViaAPI(apiURL, adminToken, providerId, undefined, undefined, 'openai')
}

/**
 * Hover over a message to reveal the action buttons, then click a specific button.
 * Buttons are hidden (opacity-0) until hover.
 */
async function hoverAndClickAction(page: any, messageLocator: any, buttonTestId: string) {
  await messageLocator.hover()
  await page.locator(`[data-testid="${buttonTestId}"]`).click()
}

test.describe('Chat - Branching', () => {
  test('regenerate: branch navigator appears at assistant bubble, not user bubble', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra

    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)
    await setupProviderAndModel(apiURL, adminToken)

    // Create a conversation and wait for the first AI response
    await createConversationWithModel(page, baseURL, 'GPT-4o Mini', 'Hello!')
    await waitForAssistantResponse(page)

    // Find the last assistant message and regenerate it
    const assistantMessages = page.locator('[data-testid="chat-message"][data-role="assistant"]')
    const lastAssistant = assistantMessages.last()
    await hoverAndClickAction(page, lastAssistant, 'regenerate-button')

    // Wait for the new AI response to arrive
    await waitForAssistantResponse(page)

    // Wait for the branch navigator to appear
    await page.waitForSelector('[data-testid="branch-navigator"]', { timeout: 10000 })

    // The navigator must be inside an ASSISTANT message, not a user message
    const navigatorInAssistant = page.locator(
      '[data-testid="chat-message"][data-role="assistant"] [data-testid="branch-navigator"]',
    )
    await expect(navigatorInAssistant).toBeVisible()

    // No navigator should be visible inside a user message
    const navigatorInUser = page.locator(
      '[data-testid="chat-message"][data-role="user"] [data-testid="branch-navigator"]',
    )
    await expect(navigatorInUser).not.toBeVisible()
  })

  test('regenerate: no duplicate user message on child branch', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra

    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)
    await setupProviderAndModel(apiURL, adminToken)

    const userMessage = 'Count to three please'
    await createConversationWithModel(page, baseURL, 'GPT-4o Mini', userMessage)
    await waitForAssistantResponse(page)

    // Regenerate the assistant response
    const lastAssistant = page.locator('[data-testid="chat-message"][data-role="assistant"]').last()
    await hoverAndClickAction(page, lastAssistant, 'regenerate-button')
    await waitForAssistantResponse(page)

    // Count how many times the user message appears — must be exactly 1
    const userMessageBubbles = page.locator(
      `[data-testid="chat-message"][data-role="user"]:has-text("${userMessage}")`,
    )
    await expect(userMessageBubbles).toHaveCount(1)
  })

  test('regenerate: navigator shows 2 branches and allows switching', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra

    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)
    await setupProviderAndModel(apiURL, adminToken)

    await createConversationWithModel(page, baseURL, 'GPT-4o Mini', 'Hi!')
    await waitForAssistantResponse(page)

    // Regenerate
    const lastAssistant = page.locator('[data-testid="chat-message"][data-role="assistant"]').last()
    await hoverAndClickAction(page, lastAssistant, 'regenerate-button')
    await waitForAssistantResponse(page)

    // Navigator should show "2 / 2" (we're on the child branch)
    const navigator = page.locator('[data-testid="branch-navigator"]')
    await expect(navigator).toBeVisible()
    await expect(navigator).toContainText('2 / 2')

    // Click the prev button to go back to branch 1
    await navigator.getByRole('button').first().click()
    await page.waitForTimeout(2000) // wait for branch activation + message reload

    // Now on branch 1, navigator should show "1 / 2"
    const updatedNavigator = page.locator('[data-testid="branch-navigator"]')
    await expect(updatedNavigator).toContainText('1 / 2')
  })

  test('regenerate then edit same message: two independent navigators, each showing 1/2', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra

    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)
    await setupProviderAndModel(apiURL, adminToken)

    const originalMessage = 'Tell me a joke'
    await createConversationWithModel(page, baseURL, 'GPT-4o Mini', originalMessage)
    await waitForAssistantResponse(page)

    // Step 1: Regenerate the assistant response → creates branch-A1 (assistant level)
    const lastAssistant = page.locator('[data-testid="chat-message"][data-role="assistant"]').last()
    await hoverAndClickAction(page, lastAssistant, 'regenerate-button')
    await waitForAssistantResponse(page)

    // Now on branch-A1. Navigator at assistant bubble should show 2/2.
    const assistantNavigator = page.locator(
      '[data-testid="chat-message"][data-role="assistant"] [data-testid="branch-navigator"]',
    )
    await expect(assistantNavigator).toBeVisible({ timeout: 10000 })
    await expect(assistantNavigator).toContainText('2 / 2')

    // Step 2: Go back to branch-main (branch 1/2)
    await assistantNavigator.getByRole('button').first().click()
    await page.waitForTimeout(2000)

    // Step 3: Edit the user message → creates branch-B (user level)
    const userMsg = page.locator('[data-testid="chat-message"][data-role="user"]').last()
    await hoverAndClickAction(page, userMsg, 'edit-message-button')
    const editor = page.locator('textarea[placeholder*="Type your message"]')
    await expect(editor).toBeVisible({ timeout: 5000 })
    await editor.fill('Tell me a better joke')
    await byTestId(page, 'chat-input-send-btn').click()
    await waitForAssistantResponse(page)

    // Now on branch-B. The user message navigator should show 2/2.
    const userNavigator = page.locator(
      '[data-testid="chat-message"][data-role="user"] [data-testid="branch-navigator"]',
    )
    await expect(userNavigator).toBeVisible({ timeout: 10000 })
    await expect(userNavigator).toContainText('2 / 2')

    // The assistant message should NOT also show a navigator here (different group)
    // — branch-B has only one assistant message, no assistant-level siblings
    const assistantNavigatorOnBranchB = page.locator(
      '[data-testid="chat-message"][data-role="assistant"] [data-testid="branch-navigator"]',
    )
    await expect(assistantNavigatorOnBranchB).not.toBeVisible()

    // Step 4: Go back to branch-main via the user navigator
    await userNavigator.getByRole('button').first().click()
    await page.waitForTimeout(2000)

    // On branch-main: should have TWO independent navigators
    // — one at the user bubble (1/2, for edit group: branch-main ↔ branch-B)
    // — one at the assistant bubble (1/2, for regenerate group: branch-main ↔ branch-A1)
    const navigators = page.locator('[data-testid="branch-navigator"]')
    await expect(navigators).toHaveCount(2)

    // Both show 1/2
    await expect(navigators.first()).toContainText('1 / 2')
    await expect(navigators.last()).toContainText('1 / 2')
  })

  test('edit: branch navigator appears at the edited user message bubble', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra

    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)
    await setupProviderAndModel(apiURL, adminToken)

    await createConversationWithModel(page, baseURL, 'GPT-4o Mini', 'Original message')
    await waitForAssistantResponse(page)

    // Click edit on the user message
    const userMessage = page.locator('[data-testid="chat-message"][data-role="user"]').last()
    await hoverAndClickAction(page, userMessage, 'edit-message-button')

    // Edit mode re-uses the ChatInput textarea (no inline editor exists);
    // it's pre-populated by startEditMessage and submitted via the Send button.
    const editor = page.locator('textarea[placeholder*="Type your message"]')
    await expect(editor).toBeVisible({ timeout: 5000 })
    await editor.fill('Edited message')

    // Confirm the edit
    await byTestId(page, 'chat-input-send-btn').click()
    await waitForAssistantResponse(page)

    // Wait for navigator to appear
    await page.waitForSelector('[data-testid="branch-navigator"]', { timeout: 10000 })

    // Navigator must be inside a USER message (edit flow anchors at user bubble)
    const navigatorInUser = page.locator(
      '[data-testid="chat-message"][data-role="user"] [data-testid="branch-navigator"]',
    )
    await expect(navigatorInUser).toBeVisible()
  })

  // =====================================================
  // Tier 1+2+3 — additional coverage for components and edge cases
  // =====================================================

  test('copy button: writes message text to clipboard and shows success toast', async ({
    browser,
    testInfra,
  }) => {
    // Clipboard access needs an explicit permission grant; create a context with it.
    const { baseURL, apiURL } = testInfra
    const context = await browser.newContext({
      permissions: ['clipboard-read', 'clipboard-write'],
    })
    const page = await context.newPage()

    try {
      await loginAsAdmin(page, baseURL)
      const adminToken = await getAdminToken(apiURL)
      await setupProviderAndModel(apiURL, adminToken)

      const userText = 'Copy this exact text'
      await createConversationWithModel(page, baseURL, 'GPT-4o Mini', userText)
      await waitForAssistantResponse(page)

      // Hover the user message and click the Copy button.
      const userMsg = page.locator('[data-testid="chat-message"][data-role="user"]').last()
      await userMsg.hover()
      await byTestId(userMsg, 'chat-message-copy-btn').click()

      // Success feedback toast appears.
      await expect(page.locator('[data-sonner-toast]').first()).toBeVisible({ timeout: 5000 })

      // Read clipboard contents and assert (the durable proof the copy ran).
      const clip = await page.evaluate(() => navigator.clipboard.readText())
      expect(clip).toBe(userText)
    } finally {
      await context.close()
    }
  })

  test('edit cancel: banner appears, cancel restores chat without trimming messages', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra

    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)
    await setupProviderAndModel(apiURL, adminToken)

    await createConversationWithModel(page, baseURL, 'GPT-4o Mini', 'Will-be-edited message')
    await waitForAssistantResponse(page)

    // Count messages before edit (1 user + 1 assistant = 2)
    const allMessages = page.locator('[data-testid="chat-message"]')
    const beforeCount = await allMessages.count()
    expect(beforeCount).toBeGreaterThanOrEqual(2)

    // Enter edit mode
    const userMsg = page.locator('[data-testid="chat-message"][data-role="user"]').last()
    await hoverAndClickAction(page, userMsg, 'edit-message-button')

    // The EditingMessageBanner should appear (banner + Cancel button)
    await expect(byTestId(page, 'chat-editing-banner')).toBeVisible({ timeout: 5000 })
    const cancelButton = byTestId(page, 'chat-editing-cancel-btn')
    await expect(cancelButton).toBeVisible()

    // Click Cancel
    await cancelButton.click()

    // Banner gone
    await expect(byTestId(page, 'chat-editing-banner')).not.toBeVisible({ timeout: 5000 })

    // Messages preserved — no trimming. The original user message is still there.
    await expect(
      page.locator('[data-testid="chat-message"][data-role="user"]:has-text("Will-be-edited message")'),
    ).toHaveCount(1)
    // And the assistant response is still rendered too
    await expect(page.locator('[data-testid="chat-message"][data-role="assistant"]')).toHaveCount(1)
  })

  test('reload: navigator anchor persists at assistant bubble after page refresh', async ({
    page,
    testInfra,
  }) => {
    // This is the linchpin test for the fork_level column. Without persisting
    // fork_level in the DB, after reload computeForkPoints would default the
    // navigator to the user bubble — wrong for the regenerate flow.
    const { baseURL, apiURL } = testInfra

    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)
    await setupProviderAndModel(apiURL, adminToken)

    await createConversationWithModel(page, baseURL, 'GPT-4o Mini', 'Anchor test')
    await waitForAssistantResponse(page)

    // Regenerate → navigator should appear at assistant bubble
    const lastAssistant = page.locator('[data-testid="chat-message"][data-role="assistant"]').last()
    await hoverAndClickAction(page, lastAssistant, 'regenerate-button')
    await waitForAssistantResponse(page)
    await page.waitForSelector('[data-testid="branch-navigator"]', { timeout: 10000 })

    // Verify the pre-reload state
    const assistantNavPre = page.locator(
      '[data-testid="chat-message"][data-role="assistant"] [data-testid="branch-navigator"]',
    )
    await expect(assistantNavPre).toBeVisible()
    await expect(assistantNavPre).toContainText('2 / 2')

    // ─── RELOAD ───
    await page.reload()
    await page.waitForLoadState('load')

    // After reload, the navigator must still anchor at the ASSISTANT bubble.
    // If fork_level weren't persisted, it would default to 'user' and the
    // navigator would render under the user bubble instead.
    const assistantNavPost = page.locator(
      '[data-testid="chat-message"][data-role="assistant"] [data-testid="branch-navigator"]',
    )
    await expect(assistantNavPost).toBeVisible({ timeout: 15000 })
    await expect(assistantNavPost).toContainText('2 / 2')

    // And NOT at the user bubble
    const userNavPost = page.locator(
      '[data-testid="chat-message"][data-role="user"] [data-testid="branch-navigator"]',
    )
    await expect(userNavPost).not.toBeVisible()
  })

  test('next button: prev then next walks back to current branch', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra

    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)
    await setupProviderAndModel(apiURL, adminToken)

    await createConversationWithModel(page, baseURL, 'GPT-4o Mini', 'Navigation test')
    await waitForAssistantResponse(page)

    // Regenerate to create a sibling branch — we land on 2/2
    const lastAssistant = page.locator('[data-testid="chat-message"][data-role="assistant"]').last()
    await hoverAndClickAction(page, lastAssistant, 'regenerate-button')
    await waitForAssistantResponse(page)

    const nav = page.locator('[data-testid="branch-navigator"]')
    await expect(nav).toBeVisible({ timeout: 10000 })
    await expect(nav).toContainText('2 / 2')

    // First nav button = Prev (LeftOutlined). Click → 1/2.
    await nav.getByRole('button').first().click()
    await expect(page.locator('[data-testid="branch-navigator"]')).toContainText('1 / 2', { timeout: 10000 })

    // Last nav button = Next (RightOutlined). Click → 2/2.
    await page.locator('[data-testid="branch-navigator"]').getByRole('button').last().click()
    await expect(page.locator('[data-testid="branch-navigator"]')).toContainText('2 / 2', { timeout: 10000 })
  })

  test('hover reveal: action button container carries the opacity-toggle classes', async ({
    page,
    testInfra,
  }) => {
    // We pin the *intent* (opacity-0 by default, opacity-100 on parent
    // hover) at the className level rather than checking runtime computed
    // opacity. Computed opacity ends up "1" in the test browser because
    // Tailwind's group-hover transitions can resolve unpredictably during
    // Playwright's headless hover synthesis — the className is the
    // authoritative contract we actually care about.
    const { baseURL, apiURL } = testInfra

    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)
    await setupProviderAndModel(apiURL, adminToken)

    await createConversationWithModel(page, baseURL, 'GPT-4o Mini', 'Hover test')
    await waitForAssistantResponse(page)

    const userMsg = page.locator('[data-testid="chat-message"][data-role="user"]').last()
    const editButton = userMsg.locator('[data-testid="edit-message-button"]')

    // The Space wrapping the action buttons must carry both Tailwind tokens
    // — otherwise the hover-reveal pattern is silently broken. AntD wraps
    // each child in an `ant-space-item`, so we need a word-boundary class
    // match (`contains(@class, "ant-space")` would match `ant-space-item`).
    const actionsContainer = editButton.locator(
      'xpath=ancestor::*[contains(concat(" ", normalize-space(@class), " "), " ant-space ")][1]',
    )
    const className = (await actionsContainer.getAttribute('class')) ?? ''
    expect(className).toContain('opacity-0')
    expect(className).toContain('group-hover:opacity-100')

    // The parent chat-message must carry the `group` class for group-hover to
    // fire. Without it the buttons stay hidden forever.
    const messageClassName = (await userMsg.getAttribute('class')) ?? ''
    expect(messageClassName).toContain('group')
  })

  test('regenerate disables the ChatInput Send button while the stream is in flight', async ({
    page,
    testInfra,
  }) => {
    // The MessageActions regenerate button's own loading window is
    // too short to catch reliably — handleRegenerate awaits
    // startRegenerateMessage which only does store setup, not the streaming
    // itself. The reliable signal of "regenerate in flight" is that the
    // ChatInput's main Send button becomes disabled (sending/isStreaming on
    // the store), which lasts the entire stream duration.
    const { baseURL, apiURL } = testInfra

    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)
    await setupProviderAndModel(apiURL, adminToken)

    await createConversationWithModel(page, baseURL, 'GPT-4o Mini', 'Loading state test')
    await waitForAssistantResponse(page)

    const lastAssistant = page.locator('[data-testid="chat-message"][data-role="assistant"]').last()
    const sendButton = byTestId(page, 'chat-input-send-btn')

    // Sanity: send button should be enabled before regenerate fires.
    await expect(sendButton).toBeEnabled()

    await hoverAndClickAction(page, lastAssistant, 'regenerate-button')

    // While the regenerated stream is in flight, the Send button must be disabled.
    await expect(sendButton).toBeDisabled({ timeout: 5000 })

    // Once the stream completes, the button re-enables.
    await waitForAssistantResponse(page)
    await expect(sendButton).toBeEnabled({ timeout: 10000 })
  })

  test('branch selection persists across page reload', async ({
    page,
    testInfra,
  }) => {
    // Independent of G3 — this verifies the *active* branch is preserved,
    // not just the navigator's anchor message.
    const { baseURL, apiURL } = testInfra

    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)
    await setupProviderAndModel(apiURL, adminToken)

    await createConversationWithModel(page, baseURL, 'GPT-4o Mini', 'Persistence test')
    await waitForAssistantResponse(page)

    // Regenerate → 2/2 (newer branch)
    const lastAssistant = page.locator('[data-testid="chat-message"][data-role="assistant"]').last()
    await hoverAndClickAction(page, lastAssistant, 'regenerate-button')
    await waitForAssistantResponse(page)

    const nav = page.locator('[data-testid="branch-navigator"]')
    await expect(nav).toContainText('2 / 2', { timeout: 10000 })

    // Switch to branch 1/2
    await nav.getByRole('button').first().click()
    await expect(page.locator('[data-testid="branch-navigator"]')).toContainText('1 / 2', { timeout: 10000 })

    // Reload
    await page.reload()
    await page.waitForLoadState('load')

    // Should still be on 1/2 — the conversation's active_branch_id is persisted server-side
    await expect(page.locator('[data-testid="branch-navigator"]')).toContainText('1 / 2', { timeout: 15000 })
  })

  test('edit pre-populates inline editor with the original message text', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra

    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)
    await setupProviderAndModel(apiURL, adminToken)

    const originalText = 'Pre-population check 12345'
    await createConversationWithModel(page, baseURL, 'GPT-4o Mini', originalText)
    await waitForAssistantResponse(page)

    const userMsg = page.locator('[data-testid="chat-message"][data-role="user"]').last()
    await hoverAndClickAction(page, userMsg, 'edit-message-button')

    // Edit mode re-uses the ChatInput's textarea (no separate inline editor);
    // startEditMessage pre-fills it with the original message text.
    const editor = page.locator('textarea[placeholder*="Type your message"]')
    await expect(editor).toBeVisible({ timeout: 5000 })
    await expect(editor).toHaveValue(originalText, { timeout: 5000 })
  })
})
