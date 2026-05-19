import { test, expect } from '../../fixtures/test-context'
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
    const editor = page.locator('textarea').last()
    await expect(editor).toBeVisible({ timeout: 5000 })
    await editor.fill('Tell me a better joke')
    await page.getByRole('button', { name: 'Save & Submit' }).click()
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

    // Wait for inline editor to appear and update the text
    const editor = page.locator('textarea').last()
    await expect(editor).toBeVisible({ timeout: 5000 })
    await editor.fill('Edited message')

    // Confirm the edit
    await page.getByRole('button', { name: 'Save & Submit' }).click()
    await waitForAssistantResponse(page)

    // Wait for navigator to appear
    await page.waitForSelector('[data-testid="branch-navigator"]', { timeout: 10000 })

    // Navigator must be inside a USER message (edit flow anchors at user bubble)
    const navigatorInUser = page.locator(
      '[data-testid="chat-message"][data-role="user"] [data-testid="branch-navigator"]',
    )
    await expect(navigatorInUser).toBeVisible()
  })
})
