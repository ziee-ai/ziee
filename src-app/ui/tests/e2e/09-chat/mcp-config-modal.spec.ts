import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  createModelViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'
import { goToNewChatPage } from './helpers/chat-helpers'

/**
 * McpConfigModal behaviour added/refactored in feat/mcp-rewrite-v2:
 *
 * - The save-on-close path now calls `saveConversationConfig(convId, ids,
 *   serverToolsMap)` *without* the `updateAutoApproved` flag. Per the Mcp.store
 *   contract, omitting that flag means the request body must NOT include
 *   `auto_approved_tools` (the backend uses COALESCE to preserve the stored value).
 * - Approval changes happen via the in-conversation tool-approval card
 *   ("Approve for this conversation"), which uses a different code path —
 *   covered separately in `mcp-tool-approval-optimistic.spec.ts`.
 *
 * The modal also exposes server-level toggles and per-tool checkboxes inside a
 * per-server Collapse, but tool listings require a live MCP server to call
 * `tools/list` against, which is out of scope for pure-UI E2E tests.
 */

test.describe('MCP Config Modal — save semantics', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)

    const token = await getAdminToken(page)
    const providerId = await createProviderViaAPI(apiURL, token, 'OpenAI', 'openai')
    await assignProviderToAdministratorsGroup(apiURL, token, providerId)
    await createModelViaAPI(apiURL, token, providerId, undefined, undefined, 'openai')
  })

  test('modal opens with title "MCP Configuration" and renders footer buttons', async ({
    page,
    testInfra,
  }) => {
    await goToNewChatPage(page, testInfra.baseURL)
    await openMcpConfigModal(page)

    await expect(page.locator('.ant-modal-title:has-text("MCP Configuration")')).toBeVisible({
      timeout: 5000,
    })
    await expect(page.locator('.ant-modal button:has-text("Save as Default")')).toBeVisible()
    // Either "Close" (new conversation) or "Save & Close" (existing) — assert one is there
    const primaryButton = page.locator('.ant-modal button.ant-btn-primary')
    await expect(primaryButton).toBeVisible()
    const primaryText = await primaryButton.textContent()
    expect(primaryText).toMatch(/Close|Save & Close/)
  })

  test('save-on-close in existing conversation omits auto_approved_tools from request body', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    const token = await getAdminToken(page)

    // 1. Create a conversation via API so we have a real conversationId
    const conv = await page.request.post(`${apiURL}/api/conversations`, {
      headers: { Authorization: `Bearer ${token}` },
      data: { title: 'config-modal-test' },
    })
    const convBody = await conv.json()
    const conversationId: string = convBody.id

    // 2. Navigate to the conversation (so McpStore.currentConversationId is set)
    await page.goto(`${baseURL}/chat/${conversationId}`)
    await page.waitForLoadState('load')
    await page.waitForTimeout(1000)

    // 3. Capture the PUT /mcp-settings body
    const capturedBodies: unknown[] = []
    await page.route(
      `**/api/conversations/${conversationId}/mcp-settings`,
      async (route, req) => {
        if (req.method() === 'PUT') {
          try {
            capturedBodies.push(JSON.parse(req.postData() || '{}'))
          } catch {
            /* ignore */
          }
        }
        await route.continue()
      },
    )

    // 4. Open and immediately close the modal (auto-save fires on close)
    await openMcpConfigModal(page)
    await expect(page.locator('.ant-modal-title:has-text("MCP Configuration")')).toBeVisible()

    await page.click('.ant-modal button.ant-btn-primary')
    await expect(page.locator('.ant-modal-title:has-text("MCP Configuration")')).not.toBeVisible({
      timeout: 5000,
    })

    // 5. Assert: at least one PUT happened AND none of them include
    //    `auto_approved_tools` (because we never touched approvals).
    expect(capturedBodies.length).toBeGreaterThanOrEqual(1)
    for (const body of capturedBodies) {
      expect(
        (body as Record<string, unknown>).auto_approved_tools,
        `auto_approved_tools must be omitted; got body: ${JSON.stringify(body)}`,
      ).toBeUndefined()
    }
  })
})

async function getAdminToken(page: import('@playwright/test').Page): Promise<string> {
  const authData = await page.evaluate(() => localStorage.getItem('auth-storage'))
  return JSON.parse(authData!).state.token
}

/**
 * Opens the MCP Config modal via its real UI path:
 *   Toolbar + button → dropdown → "MCP tools & servers" menu item.
 * This is the only way the modal can be opened in the app today; there is
 * no standalone toolbar button. Requires at least one enabled MCP server
 * (otherwise the McpMenuItem hides itself).
 */
async function openMcpConfigModal(page: import('@playwright/test').Page): Promise<void> {
  await page.getByRole('button', { name: 'Add attachment' }).first().click()
  await page.getByText('MCP tools & servers').first().click()
  await expect(page.locator('.ant-modal-title:has-text("MCP Configuration")')).toBeVisible({
    timeout: 5000,
  })
}
