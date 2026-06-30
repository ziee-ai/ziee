import { Page, expect } from '@playwright/test'

/**
 * Chat E2E Test Helpers
 *
 * Navigation, model selection, and message sending helpers for chat functionality
 */

// =====================================================
// Navigation Helpers
// =====================================================

export async function goToNewChatPage(page: Page, baseURL: string) {
  await page.goto(`${baseURL}/chat`)
  await page.waitForLoadState('load')
  await waitForNewChatPageLoad(page)
}

export async function waitForNewChatPageLoad(page: Page) {
  // Wait for the welcome heading
  await page.waitForSelector('text=How can I help you today?', { timeout: 30000 })
  // Wait for chat input to be visible
  await page.waitForSelector('textarea[placeholder*="Type your message"]', {
    timeout: 10000,
  })
  // Wait for the model selector trigger (Radix/shadcn Select — the kit
  // ModelSelector renders a combobox trigger with data-testid="ullm-model-select").
  await page.waitForSelector('[data-testid="ullm-model-select"]', { timeout: 10000 })

  // Wait for models to load with retry logic
  // The store's loadProviders() is async and not awaited, so we need to poll
  const maxRetries = 10
  const retryDelay = 1000 // 1 second between retries

  for (let i = 0; i < maxRetries; i++) {
    // If the app has already auto-selected a model (it does when the user has
    // exactly one accessible model), the Radix trigger shows the model name
    // (not the "Select Model" / "Loading…" placeholder) — models are loaded.
    const triggerText = (
      await page
        .locator('[data-testid="ullm-model-select"]')
        .textContent({ timeout: 500 })
        .catch(() => null)
    )?.trim()
    if (
      triggerText &&
      triggerText !== 'Select Model' &&
      triggerText !== 'Loading…'
    ) {
      return
    }

    // Open dropdown
    await page.locator('[data-testid="ullm-model-select"]').click()
    await page.waitForSelector('[role="listbox"]', { state: 'visible', timeout: 5000 })

    // Check if models are loaded (≥1 option rendered)
    const optionCount = await page.locator('[role="option"]').count()

    if (optionCount > 0) {
      // Models loaded! Close dropdown and return
      await page.keyboard.press('Escape')
      await page.waitForSelector('[role="listbox"]', { state: 'hidden', timeout: 5000 })
      return
    }

    // Models not loaded yet, close dropdown and retry
    await page.keyboard.press('Escape')
    await page.waitForSelector('[role="listbox"]', { state: 'hidden', timeout: 5000 })

    if (i < maxRetries - 1) {
      await page.waitForTimeout(retryDelay)
    }
  }

  // After all retries, the dropdown is still showing "No data". That's a valid
  // page state — the user may legitimately have no model access. Return
  // successfully and let the caller's own assertions (assertDropdownEmpty,
  // getVisibleModelsInDropdown, etc.) decide whether that's expected.
}

export async function goToChatPage(page: Page, baseURL: string, conversationId: string) {
  await page.goto(`${baseURL}/chat/${conversationId}`)
  await page.waitForLoadState('load')
  await waitForChatPageLoad(page)
}

export async function waitForChatPageLoad(page: Page) {
  // Wait for chat messages container
  await page.waitForSelector('[data-testid="chat-messages"]', { timeout: 30000 })
  // Wait for chat input to be visible
  await page.waitForSelector('textarea[placeholder*="Type your message"]', { timeout: 10000 })
}

// =====================================================
// Model Selection Helpers
// =====================================================

export async function getVisibleModelsInDropdown(page: Page): Promise<string[]> {
  // Open the Radix/shadcn Select dropdown by clicking its trigger.
  await page.locator('[data-testid="ullm-model-select"]').click()

  await page.waitForSelector('[role="listbox"]', { state: 'visible', timeout: 5000 })

  // Get all option labels (model display names)
  const options = await page.getByRole('option').allTextContents()

  // Close dropdown
  await page.keyboard.press('Escape')
  await page.waitForSelector('[role="listbox"]', { state: 'hidden', timeout: 5000 })

  return options.map((o) => o.trim())
}

export async function selectModelInDropdown(
  page: Page,
  modelName: string
): Promise<void> {
  // Check if the model is already selected. The selected-value
  // element only exists once the user has picked a model — in fresh
  // chat views the element is absent. `textContent()` without a
  // short timeout would block for the full default (10s), so we
  // catch + fall through to the open-dropdown flow.
  const currentSelection = (
    await page
      .locator('[data-testid="ullm-model-select"]')
      .textContent({ timeout: 1000 })
      .catch(() => null)
  )?.trim()

  if (currentSelection === modelName) {
    // Model already selected, nothing to do
    return
  }

  // Model not selected, open the Radix/shadcn dropdown and select it
  await page.locator('[data-testid="ullm-model-select"]').click()

  // Wait for dropdown to appear
  await page.waitForSelector('[role="listbox"]', { state: 'visible', timeout: 5000 })

  // Click the option with the model name (Radix renders options with role="option").
  await page.getByRole('option', { name: modelName, exact: false }).first().click()

  // Radix auto-closes on select; best-effort dismiss otherwise.
  await page.waitForSelector('[role="listbox"]', { state: 'hidden', timeout: 5000 }).catch(async () => {
    await page.keyboard.press('Escape')
    await page.waitForSelector('[role="listbox"]', { state: 'hidden', timeout: 5000 })
  })
}

export async function assertModelVisibleInDropdown(
  page: Page,
  modelName: string
): Promise<void> {
  const visibleModels = await getVisibleModelsInDropdown(page)
  expect(visibleModels).toContain(modelName)
}

export async function assertModelNotVisibleInDropdown(
  page: Page,
  modelName: string
): Promise<void> {
  const visibleModels = await getVisibleModelsInDropdown(page)
  expect(visibleModels).not.toContain(modelName)
}

export async function assertDropdownEmpty(page: Page): Promise<void> {
  // Open the Radix/shadcn model selector dropdown
  await page.locator('[data-testid="ullm-model-select"]').click()

  // Give the portal a beat to render, then assert no options exist.
  // Radix renders an empty listbox (or none) when there are zero items.
  await page.waitForTimeout(500)
  const optionCount = await page.getByRole('option').count()
  expect(optionCount).toBe(0)

  // Close dropdown
  await page.keyboard.press('Escape')
}

// =====================================================
// Message Sending Helpers
// =====================================================

export async function sendChatMessage(
  page: Page,
  message: string,
  waitForResponse = true
): Promise<void> {
  const sendButton = page.getByRole('button', { name: 'Send message' })
  const textarea = page.locator('textarea[placeholder*="Type your message"]')

  // Retry logic: sometimes the send button click is ignored if streaming is still active
  // Even though the button is enabled, the onClick handler returns early if isStreaming=true
  let attempts = 0
  const maxAttempts = 3

  while (attempts < maxAttempts) {
    attempts++

    // Wait for send button to be enabled
    await expect(sendButton).toBeEnabled({ timeout: 30000 })

    // Type message in textarea
    await textarea.fill(message)

    // Click send button
    await sendButton.click()

    // Check if textarea cleared (indicates message was sent successfully)
    try {
      await expect(textarea).toHaveValue('', { timeout: 3000 })
      // Success! Message was sent
      break
    } catch (error) {
      // Textarea still has text - send was ignored
      if (attempts < maxAttempts) {
        console.log(`Send attempt ${attempts} failed, retrying after 1s...`)
        await page.waitForTimeout(1000)
      } else {
        // Final attempt failed, throw the error
        throw error
      }
    }
  }

  if (waitForResponse) {
    // Wait for assistant response to appear
    await waitForAssistantResponse(page)
  }
}

export async function waitForAssistantResponse(page: Page): Promise<void> {
  // Wait for a new message with role="assistant" to appear
  await page.waitForSelector('[data-role="assistant"]', { timeout: 30000 })

  // Wait longer for streaming state to fully clear in the store
  // Real AI streaming takes time to complete and update state
  await page.waitForTimeout(2000)
}

export async function getLastMessageContent(page: Page): Promise<string> {
  // Get the last message content
  const messages = page.locator('[data-testid="chat-message"]')
  const lastMessage = messages.last()
  return await lastMessage.textContent() || ''
}

export async function assertMessageInHistory(
  page: Page,
  messageContent: string
): Promise<void> {
  const message = page.locator(`[data-testid="chat-message"]:has-text("${messageContent}")`)
  await expect(message).toBeVisible()
}

// =====================================================
// Conversation Creation Helpers
// =====================================================

export async function createConversationWithModel(
  page: Page,
  baseURL: string,
  modelName: string,
  initialMessage: string
): Promise<string> {
  await goToNewChatPage(page, baseURL)

  // Select the model
  await selectModelInDropdown(page, modelName)

  // Send the initial message
  const textarea = page.locator('textarea[placeholder*="Type your message"]')
  await textarea.fill(initialMessage)

  // Click send button
  const sendButton = page.getByRole('button', { name: 'Send message' })
  await sendButton.click()

  // Wait for navigation to conversation page
  await page.waitForURL(/\/chat\/[a-f0-9-]+/, { timeout: 10000 })

  // Wait for conversation page to load
  await waitForChatPageLoad(page)

  // Extract conversation ID from URL
  const url = page.url()
  const match = url.match(/\/chat\/([a-f0-9-]+)/)
  if (!match) {
    throw new Error('Failed to extract conversation ID from URL')
  }

  return match[1]
}

// =====================================================
// Accessibility Helpers
// =====================================================

export async function assertChatPageAccessibility(page: Page): Promise<void> {
  // Check for main landmarks
  await expect(page.locator('main, [role="main"]')).toBeVisible()

  // Check textarea has proper label (use getByRole to get only the visible textbox)
  const textarea = page.getByRole('textbox')
  const ariaLabel = await textarea.getAttribute('aria-label')
  const placeholder = await textarea.getAttribute('placeholder')
  expect(ariaLabel || placeholder).toBeTruthy()

  // Check model selector has proper label
  const select = page.locator('[data-testid="ullm-model-select"]')
  await expect(select).toBeVisible()
}
