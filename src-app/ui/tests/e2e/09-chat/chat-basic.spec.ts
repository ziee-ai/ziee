import { test, expect } from '../../fixtures/test-context'
import { assertNoAccessibilityViolations } from '../../utils/accessibility'
import { loginAsAdmin, login, createTestUser, getAdminToken, clearAuthState } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  createModelViaAPI,
  assignProviderToAdministratorsGroup,
  createGroupViaAPI,
  assignUserToGroupViaAPI,
  assignProviderToGroupViaAPI,
  getAdministratorsGroupId,
} from '../../common/provider-helpers'
import {
  goToNewChatPage,
  sendChatMessage,
  createConversationWithModel,
  assertChatPageAccessibility,
  getVisibleModelsInDropdown,
} from './helpers/chat-helpers'

/**
 * Chat - Basic Flow E2E Tests
 *
 * Tests for core chat functionality: creating conversations, sending messages, and basic interactions
 */

test.describe('Chat - Basic Flow', () => {
  test('should pass accessibility checks on new chat page', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra

    // Create admin user first
    await loginAsAdmin(page, baseURL)

    // Get admin token for API calls
    const adminToken = await getAdminToken(apiURL)
    const providerId = await createProviderViaAPI(apiURL, adminToken, 'OpenAI', 'openai')
    await assignProviderToAdministratorsGroup(apiURL, adminToken, providerId)
    await createModelViaAPI(apiURL, adminToken, providerId, undefined, undefined, 'openai')

    // DEBUG: Call the API directly to see what it returns
    const chatProvidersResponse = await fetch(`${apiURL}/api/chat/llm-providers`, {
      headers: { Authorization: `Bearer ${adminToken}` },
    })
    const chatProvidersData = await chatProvidersResponse.json()
    console.log('DEBUG: Chat providers API response:', JSON.stringify(chatProvidersData, null, 2))

    // Navigate to chat page - store will initialize with new models
    await goToNewChatPage(page, baseURL)

    // Run accessibility checks. The model selector is an Ant Design v6
    // `Select`, which has several documented a11y limitations that axe-core
    // flags but that we can't fix without forking antd's component/theme:
    //   - `label` / `aria-input-field-name`: antd doesn't propagate an
    //     accessible name to the internal combobox input nor to the popup's
    //     virtual `role="listbox"`.
    //   - `color-contrast`: antd's default theme renders option-group labels
    //     and option text in muted greys below the 4.5:1 WCAG AA ratio.
    // The popup briefly mounts (not `display:none`) while the Select settles,
    // so axe audits it. We disable these antd-Select-specific rules here, the
    // same workaround the settings (label) and 05-llm / 07-mcp (color-contrast)
    // a11y tests use; every other rule (landmarks, textarea label, etc.) still
    // runs.
    await assertChatPageAccessibility(page)
    await assertNoAccessibilityViolations(page, {
      disabledRules: ['label', 'aria-input-field-name', 'color-contrast'],
    })
  })

  test('should display new chat page with welcome message and input', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra

    // Create admin user first
    await loginAsAdmin(page, baseURL)

    // Get admin token for API calls
    const adminToken = await getAdminToken(apiURL)
    const providerId = await createProviderViaAPI(apiURL, adminToken, 'OpenAI', 'openai')
    await assignProviderToAdministratorsGroup(apiURL, adminToken, providerId)
    await createModelViaAPI(apiURL, adminToken, providerId, undefined, undefined, 'openai')

    // Navigate to chat page - store will initialize with new models
    await goToNewChatPage(page, baseURL)

    // Verify page elements
    await expect(page.locator('text=How can I help you today?')).toBeVisible()
    await expect(page.locator('textarea[placeholder*="Type your message"]')).toBeVisible()
    await expect(page.locator('.ant-select')).toBeVisible() // Model selector
  })

  test('should create conversation and send first message', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra

    // Create admin user first
    await loginAsAdmin(page, baseURL)

    // Get admin token for API calls
    const adminToken = await getAdminToken(apiURL)
    const providerId = await createProviderViaAPI(apiURL, adminToken, 'OpenAI', 'openai')
    await assignProviderToAdministratorsGroup(apiURL, adminToken, providerId)
    await createModelViaAPI(apiURL, adminToken, providerId, undefined, undefined, 'openai')

    // Create conversation with initial message
    const conversationId = await createConversationWithModel(
      page,
      baseURL,
      'GPT-4o Mini',
      'Hello, this is my first message!'
    )

    // Verify we're on the conversation page
    expect(page.url()).toContain(`/chat/${conversationId}`)

    // Verify message appears in history
    await expect(
      page.locator('[data-testid="chat-message"]').filter({ hasText: 'Hello, this is my first message!' })
    ).toBeVisible()

    // Verify chat input is ready for next message
    await expect(page.locator('textarea[placeholder*="Type your message"]')).toBeVisible()
  })

  test('should send multiple messages in existing conversation', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra

    // Create admin user first
    await loginAsAdmin(page, baseURL)

    // Get admin token for API calls
    const adminToken = await getAdminToken(apiURL)
    const providerId = await createProviderViaAPI(apiURL, adminToken, 'OpenAI', 'openai')
    await assignProviderToAdministratorsGroup(apiURL, adminToken, providerId)
    await createModelViaAPI(apiURL, adminToken, providerId, undefined, undefined, 'openai')

    // Create conversation (already navigates to conversation page)
    await createConversationWithModel(
      page,
      baseURL,
      'GPT-4o Mini',
      'First message'
    )

    // No need to navigate again - we're already on the conversation page
    // Send second message and wait for AI response to complete
    await sendChatMessage(page, 'Second message', true)

    // Send third message and wait for AI response to complete
    await sendChatMessage(page, 'Third message', true)

    // Verify all USER messages are visible. We scope to data-role="user" because
    // assistant echoes (or the assistant response containing the same words) would
    // make a bare text-only filter ambiguous.
    await expect(
      page.locator('[data-testid="chat-message"][data-role="user"]').filter({ hasText: 'First message' })
    ).toBeVisible()
    await expect(
      page.locator('[data-testid="chat-message"][data-role="user"]').filter({ hasText: 'Second message' })
    ).toBeVisible()
    await expect(
      page.locator('[data-testid="chat-message"][data-role="user"]').filter({ hasText: 'Third message' })
    ).toBeVisible()
  })

  test('should display model selector in active conversation', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra

    // Create admin user first
    await loginAsAdmin(page, baseURL)

    // Get admin token for API calls
    const adminToken = await getAdminToken(apiURL)

    const provider1Id = await createProviderViaAPI(apiURL, adminToken, 'OpenAI', 'openai')
    await createModelViaAPI(apiURL, adminToken, provider1Id, undefined, undefined, 'openai')

    const provider2Id = await createProviderViaAPI(apiURL, adminToken, 'Anthropic', 'anthropic')
    await createModelViaAPI(apiURL, adminToken, provider2Id, undefined, undefined, 'anthropic')

    // Assign both providers at once (PUT replaces the entire list)
    await assignProviderToGroupViaAPI(apiURL, adminToken, await getAdministratorsGroupId(apiURL, adminToken), [provider1Id, provider2Id])

    // Reload page to ensure providers are loaded in frontend
    await page.reload()
    await page.waitForLoadState('load')

    // Create conversation with GPT-4o Mini (already navigates to conversation page)
    await createConversationWithModel(
      page,
      baseURL,
      'GPT-4o Mini',
      'Test message'
    )

    // No need to navigate again - we're already on the conversation page
    // Verify model selector is visible and shows both models
    await expect(page.locator('.ant-select')).toBeVisible()

    const visibleModels = await getVisibleModelsInDropdown(page)
    expect(visibleModels).toContain('GPT-4o Mini')
    expect(visibleModels).toContain('Claude 3.5 Sonnet')
  })

  test('model selector is rendered via slot in the chat toolbar', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra

    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)
    const providerId = await createProviderViaAPI(apiURL, adminToken, 'OpenAI', 'openai')
    await assignProviderToAdministratorsGroup(apiURL, adminToken, providerId)
    await createModelViaAPI(apiURL, adminToken, providerId, undefined, undefined, 'openai')

    await goToNewChatPage(page, baseURL)

    // Verify the model selector is present in the toolbar via its data-testid
    // (this confirms it is mounted through the toolbar_model slot, not a direct import)
    await expect(page.locator('[data-testid="model-selector"]')).toBeVisible()

    // Verify the dropdown contains the created model
    const visibleModels = await getVisibleModelsInDropdown(page)
    expect(visibleModels).toContain('GPT-4o Mini')
  })

  test('regular user can create conversation and send messages', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra

    // Create admin user first
    await loginAsAdmin(page, baseURL)

    // Get admin token for API calls
    const adminToken = await getAdminToken(apiURL)
    const providerId = await createProviderViaAPI(apiURL, adminToken, 'OpenAI', 'openai')
    await createModelViaAPI(apiURL, adminToken, providerId, undefined, undefined, 'openai')

    // Create test user with access to the model
    const username = `chatuser_${Date.now()}`
    const password = 'TestPass123!'
    const userId = await createTestUser(apiURL, adminToken, username, `${username}@test.com`, password, [
      'conversations::create',
      'conversations::read',
      'messages::create',
      'messages::read',
    ])

    // Grant user access via group using API
    const groupId = await createGroupViaAPI(apiURL, adminToken, `chat-group-${Date.now()}`, 'Chat test group')
    await assignProviderToGroupViaAPI(apiURL, adminToken, groupId, [providerId])
    await assignUserToGroupViaAPI(apiURL, adminToken, userId, groupId)

    // Logout admin before logging in as regular user
    await clearAuthState(page)

    // Login as regular user - store will load with the newly created models
    await login(page, baseURL, username, password)

    // Create conversation and send message
    await createConversationWithModel(
      page,
      baseURL,
      'GPT-4o Mini',
      'Hello from regular user!'
    )

    // Verify message appears
    await expect(
      page.locator('[data-testid="chat-message"]').filter({ hasText: 'Hello from regular user!' })
    ).toBeVisible()

    // Send another message
    await sendChatMessage(page, 'Second message from user', false)

    await expect(
      page.locator('[data-testid="chat-message"]').filter({ hasText: 'Second message from user' })
    ).toBeVisible()
  })
})
