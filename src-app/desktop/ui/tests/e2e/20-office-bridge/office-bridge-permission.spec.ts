// TEST-84 [negative-perm] — office_bridge is desktop-only + permission-gated on
// `office_bridge::use`. A user LACKING the perm must never see the office UI, even
// for a (seeded/leaked) `list_open_documents` tool_result: the tool-result-card's
// `contentMatch` gate + component gate + the panel gate all key on the perm, so the
// office card is NOT claimed — the block falls through to the default renderer. This
// asserts card ABSENCE for a restricted user; the positive control (same seeded flow,
// permitted user) asserts the card IS present — so the absence is the gate, not a
// broken seed (non-vacuous).
import type { Page } from '@playwright/test'
import { test, expect } from '../../fixtures/test-context'
import { installTauriMock } from '../helpers/tauri-mock'
import { createTestUser } from '../../../../../ui/tests/common/auth-helpers'
import {
  createProviderViaAPI,
  createModelViaAPI,
  createGroupViaAPI,
  assignUserToGroupViaAPI,
  assignProviderToGroupViaAPI,
} from '../../../../../ui/tests/common/provider-helpers'
import {
  mockChatTokenStream,
  startedEvent,
  completeEvent,
  mockGetMessages,
  mockUserMessage,
  type MockMessageContent,
} from '../../../../../ui/tests/e2e/helpers/sse-mock-helpers'
import { goToNewChatPage, selectModelInDropdown } from '../../../../../ui/tests/e2e/chat/helpers/chat-helpers'
import { byTestId } from '../../../../../ui/tests/e2e/testid'

interface LoginTokens {
  access_token: string
  refresh_token: string
  expires_in?: number
  user: { id: string; username: string; email: string; permissions: string[]; is_admin: boolean; [k: string]: unknown }
}

const BASE_PERMS = [
  'profile::read', 'profile::edit',
  'conversations::create', 'conversations::read', 'conversations::edit',
  'messages::create', 'messages::read',
  'llm_models::read', 'llm_providers::read',
  // user-facing provider/model view — what the `ullm-model-select` dropdown reads.
  'user_llm_providers::read',
]

const SAMPLE_DOCS = [
  { app: 'word', name: 'Q3-Report.docx', full_name: 'C:/Users/analyst/Q3-Report.docx', path: 'C:/Users/analyst', saved: true, active: true, attach_method: 'com_get_active_object' },
  { app: 'excel', name: 'Budget.xlsx', full_name: 'C:/Users/analyst/Budget.xlsx', path: 'C:/Users/analyst', saved: false, active: false, attach_method: 'accessible_object_from_window' },
]

/** Create a permission-scoped user (no Users-group inheritance) with model access
 *  via a fresh group + provider, and mint their real login tokens. `extraPerms`
 *  adds `office_bridge::use` for the positive control. */
async function setupUser(
  apiURL: string,
  adminToken: string,
  providerId: string,
  username: string,
  extraPerms: string[],
): Promise<LoginTokens> {
  const password = 'password123'
  const userId = await createTestUser(apiURL, adminToken, username, `${username}@example.com`, password, [
    ...BASE_PERMS,
    ...extraPerms,
  ])
  const groupId = await createGroupViaAPI(apiURL, adminToken, `grp_${username}`, 'office-perm test group', [])
  await assignUserToGroupViaAPI(apiURL, adminToken, userId, groupId)
  await assignProviderToGroupViaAPI(apiURL, adminToken, groupId, [providerId])
  const resp = await fetch(`${apiURL}/api/auth/login`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ username, password }),
  })
  if (!resp.ok) throw new Error(`login ${username}: ${resp.status} ${await resp.text()}`)
  return (await resp.json()) as LoginTokens
}

/** Seed a chat whose assistant turn carries a `list_open_documents` tool_result,
 *  then send it — mirrors the office-bridge.spec seed. */
async function seedOfficeToolResult(page: Page, baseURL: string): Promise<string> {
  const toolUseId = `tu_${Math.random().toString(36).slice(2, 9)}`
  const assistantMessageId = `amsg_${Math.random().toString(36).slice(2, 9)}`
  const userMessageId = `umsg_${Math.random().toString(36).slice(2, 9)}`
  await page.route('**/api/office-bridge/documents', route =>
    route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(SAMPLE_DOCS) }),
  )
  await mockChatTokenStream(page, [[startedEvent({ userMessageId }), completeEvent()]])
  const toolUse: MockMessageContent = {
    content_type: 'tool_use',
    content: { type: 'tool_use', id: toolUseId, name: 'list_open_documents', server_id: 'office-bridge-test-server', input: {} },
  }
  const toolResult: MockMessageContent = {
    content_type: 'tool_result',
    content: {
      type: 'tool_result', tool_use_id: toolUseId, name: 'list_open_documents',
      server_id: 'office-bridge-test-server', content: `${SAMPLE_DOCS.length} open Office document(s).`,
      structured_content: { documents: SAMPLE_DOCS }, is_error: false,
    },
  }
  await mockGetMessages(page, [
    mockUserMessage({ id: userMessageId, text: 'what documents are open?' }),
    { id: assistantMessageId, role: 'assistant', contents: [toolUse, toolResult] },
  ])
  await goToNewChatPage(page, baseURL)
  await selectModelInDropdown(page, 'GPT-4o Mini')
  await page.locator('textarea[placeholder*="Type your message"]').first().fill('what documents are open?')
  await byTestId(page, 'chat-input-send-btn').click()
  await page
    .locator(`[data-testid="chat-message"][data-message-id="${assistantMessageId}"]`)
    .first()
    .waitFor({ state: 'visible', timeout: 15000 })
  return assistantMessageId
}

test.describe('office-bridge — permission gating (negative-perm)', () => {
  test.describe.configure({ retries: 1 })

  test('a user LACKING office_bridge::use does not see the office tool-result card', async ({ page, testInfra }) => {
    const apiURL = testInfra.backendURL
    const adminToken = testInfra.tokens.access_token
    const tag = Date.now().toString(36)
    const providerId = await createProviderViaAPI(apiURL, adminToken, `OpenAI ${tag}`, 'openai')
    await createModelViaAPI(apiURL, adminToken, providerId, undefined, undefined, 'openai')

    const restricted = await setupUser(apiURL, adminToken, providerId, `office_noperm_${tag}`, [])
    // Guard: genuinely lacks the perm (no surprise group inheritance).
    expect(restricted.user.permissions).not.toContain('office_bridge::use')

    await installTauriMock(page, { backendPort: testInfra.backendPort, tokens: restricted })
    const assistantMessageId = await seedOfficeToolResult(page, '')

    // The assistant turn (with the raw tool_result) DID render — non-vacuous...
    await expect(
      page.locator(`[data-testid="chat-message"][data-message-id="${assistantMessageId}"]`).first(),
    ).toBeVisible()
    // ...but the office card is ABSENT (contentMatch gated → fell through to default renderer),
    // and its "Open panel" affordance is gone.
    await expect(byTestId(page, 'office-docs-tool-result-card')).toHaveCount(0)
    await expect(byTestId(page, 'office-docs-tool-result-open-button')).toHaveCount(0)
  })

  test('positive control: a user HOLDING office_bridge::use sees the office tool-result card', async ({ page, testInfra }) => {
    const apiURL = testInfra.backendURL
    const adminToken = testInfra.tokens.access_token
    const tag = Date.now().toString(36)
    const providerId = await createProviderViaAPI(apiURL, adminToken, `OpenAI ${tag}`, 'openai')
    await createModelViaAPI(apiURL, adminToken, providerId, undefined, undefined, 'openai')

    const permitted = await setupUser(apiURL, adminToken, providerId, `office_withperm_${tag}`, ['office_bridge::use'])
    expect(permitted.user.permissions).toContain('office_bridge::use')

    await installTauriMock(page, { backendPort: testInfra.backendPort, tokens: permitted })
    await seedOfficeToolResult(page, '')

    // With the perm the office card renders (the same seeded flow) — proving the
    // restricted user's absence above is the gate, not a broken seed.
    await expect(byTestId(page, 'office-docs-tool-result-card')).toBeVisible({ timeout: 15000 })
    await expect(byTestId(page, 'office-docs-tool-result-summary')).toContainText('2 open documents')
  })
})
