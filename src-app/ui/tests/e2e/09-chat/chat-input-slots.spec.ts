import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  createModelViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'
import { goToNewChatPage } from './helpers/chat-helpers'

/**
 * E2E tests for the chat-input slot system (`chore/chat-input-slot-refactor`).
 *
 * `ChatInput` is now a thin orchestrator that renders extension-provided
 * components in named slots. These tests verify each slot is wired up and
 * that its contributors render as expected:
 *
 *   - toolbar_model      → ModelSelector (already covered by chat-basic.spec)
 *   - toolbar_plus_items → FileAttachMenuItem, AssistantMenuItem, McpMenuItem
 *   - toolbar_status     → AssistantStatusChip, McpStatusRow (conditional)
 *   - toolbar_actions    → KeyboardShortcutsHelp (always), Export (only with messages)
 *   - text_input         → TextInput (covered by chat-basic.spec via textarea visible)
 *
 * Setup per test: register the admin user, create an OpenAI provider+model
 * (so the page bootstraps cleanly), navigate to the new-chat page.
 */

async function setupChatPage(
  page: import('@playwright/test').Page,
  baseURL: string,
  apiURL: string,
) {
  await loginAsAdmin(page, baseURL)
  const adminToken = await getAdminToken(apiURL)
  const providerId = await createProviderViaAPI(apiURL, adminToken, 'OpenAI', 'openai')
  await assignProviderToAdministratorsGroup(apiURL, adminToken, providerId)
  await createModelViaAPI(apiURL, adminToken, providerId, undefined, undefined, 'openai')
  await goToNewChatPage(page, baseURL)
  return { adminToken, providerId }
}

test.describe('Chat input slot system', () => {
  test('plus dropdown opens and contains all three registered menu items', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await setupChatPage(page, baseURL, apiURL)

    // Click the "+" button (semantic: button with aria-label "Add attachment").
    await page.getByRole('button', { name: 'Add attachment' }).click()

    // The dropdown should appear; all three menu items from the three
    // extensions registered in `toolbar_plus_items` must be visible.
    await expect(page.getByText('Attach files or photos')).toBeVisible()
    await expect(page.getByText('Select assistant')).toBeVisible()
    await expect(page.getByText('MCP tools & servers')).toBeVisible()
  })

  test('selecting an assistant shows AssistantStatusChip in toolbar_status', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra

    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)
    const providerId = await createProviderViaAPI(apiURL, adminToken, 'OpenAI', 'openai')
    await assignProviderToAdministratorsGroup(apiURL, adminToken, providerId)
    await createModelViaAPI(apiURL, adminToken, providerId, undefined, undefined, 'openai')

    // Create an assistant via API so the AssistantMenu has something to pick.
    const assistantResp = await fetch(`${apiURL}/api/assistants`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${adminToken}` },
      body: JSON.stringify({
        name: 'Slot Test Assistant',
        description: 'For e2e slot test',
        instructions: 'You are a helpful slot test assistant.',
        is_template: false,
      }),
    })
    if (!assistantResp.ok) {
      throw new Error(`Failed to create assistant: ${assistantResp.status} ${await assistantResp.text()}`)
    }

    await goToNewChatPage(page, baseURL)

    // Open "+" → click "Select assistant" → the Popover with the assistant
    // appears to the right. AssistantOption is a <div onClick> containing
    // a <span> with the assistant name.
    await page.getByRole('button', { name: 'Add attachment' }).click()
    await page.getByText('Select assistant').click()

    // Wait for the assistant name to appear inside the opened popover, then click it.
    await expect(page.getByText('Slot Test Assistant')).toBeVisible()
    await page.getByText('Slot Test Assistant').click()

    // After selection, AssistantStatusChip should render with the assistant name
    // (it's a purple Tag in the status row).
    await expect(page.locator('.ant-tag').filter({ hasText: 'Slot Test Assistant' })).toBeVisible()
  })

  test('toolbar_actions slot renders keyboard shortcut tips', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await setupChatPage(page, baseURL, apiURL)

    // The KeyboardShortcutsHelp component renders a <span>Tips: Ctrl+Enter ...</span>
    // in the toolbar_actions slot. (Export is also in this slot but only appears
    // once there are messages — covered indirectly by chat-basic flow tests.)
    await expect(page.getByText(/Tips: Ctrl\+Enter to send/)).toBeVisible()
  })

  test('selecting an assistant closes the plus dropdown (PlusDropdownContext.close)', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra

    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)
    const providerId = await createProviderViaAPI(apiURL, adminToken, 'OpenAI', 'openai')
    await assignProviderToAdministratorsGroup(apiURL, adminToken, providerId)
    await createModelViaAPI(apiURL, adminToken, providerId, undefined, undefined, 'openai')

    await fetch(`${apiURL}/api/assistants`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${adminToken}` },
      body: JSON.stringify({
        name: 'Close Test Assistant',
        description: 'x',
        instructions: 'x',
        is_template: false,
      }),
    })

    await goToNewChatPage(page, baseURL)

    // Open dropdown → submenu → pick assistant. AssistantOption.onClick calls
    // selectAssistant + close() — so the parent dropdown must dismiss.
    await page.getByRole('button', { name: 'Add attachment' }).click()
    await expect(page.getByText('MCP tools & servers')).toBeVisible() // parent dropdown open
    await page.getByText('Select assistant').click()
    await expect(page.getByText('Close Test Assistant')).toBeVisible() // submenu open
    await page.getByText('Close Test Assistant').click()

    // Parent dropdown items should now be hidden — PlusDropdownContext.close fired.
    await expect(page.getByText('MCP tools & servers')).not.toBeVisible({ timeout: 3000 })
    await expect(page.getByText('Attach files or photos')).not.toBeVisible({ timeout: 3000 })
  })

  test('mcp menu item opens MCP config modal', async ({ page, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    await setupChatPage(page, baseURL, apiURL)

    // Open "+" → click "MCP tools & servers" → modal should appear.
    await page.getByRole('button', { name: 'Add attachment' }).click()
    await page.getByText('MCP tools & servers').click()

    // The MCP config modal renders an Ant Modal — assert a dialog is open.
    await expect(page.getByRole('dialog')).toBeVisible()
  })
})
