import type { Page } from '@playwright/test'
// office_bridge is desktop-only, but its panel renders inside the (web-ui core)
// chat right-panel, so this spec reuses the web-ui e2e chat/provider/SSE-mock
// harness from the shared monorepo. Phase 8 validates the runtime wiring against
// the desktop/ui playwright runner.
// Local (desktop/ui) test-context so `testInfra` reads the postgres config that
// THIS workspace's global-setup writes (desktop/ui/tests/.test-configs). Importing
// the ui-workspace context read from ui/tests/.test-configs → ENOENT. The pure
// helpers below stay cross-workspace (no path-dependent infra).
import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../../../../ui/tests/common/auth-helpers'
import {
  createProviderViaAPI,
  createModelViaAPI,
  assignProviderToAdministratorsGroup,
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

/**
 * TEST-18 [covers ITEM-14] — e2e: the office-bridge `list_open_documents`
 * tool-result card renders inline, and clicking it opens the "Open Office
 * documents" right-panel listing the enumerated documents.
 *
 * Only the SSE/tool boundary is mocked (a deterministic, no-live-LLM seed of a
 * `list_open_documents` tool_result carrying a typed `structuredContent`
 * payload, delivered via the production post-`complete` /messages reload); the
 * chat rendering, the tool-result card, and the right-panel all run for real.
 */

interface OpenDoc {
  app: 'word' | 'excel' | 'power_point'
  name: string
  full_name: string
  path?: string | null
  saved: boolean
  active: boolean
  attach_method: string
}

/** Seed an assistant turn whose `list_open_documents` tool_result carries the
 *  given open documents in its `structured_content`, then send it in a fresh
 *  chat. Mirrors the literature `seedLiteratureResult` fixture. */
async function seedOpenDocumentsResult(
  page: Page,
  baseURL: string,
  documents: OpenDoc[],
): Promise<void> {
  const toolUseId = `tu_office_${Math.random().toString(36).slice(2, 9)}`
  const assistantMessageId = `amsg_office_${Math.random().toString(36).slice(2, 9)}`
  const userMessageId = `umsg_office_${Math.random().toString(36).slice(2, 9)}`
  const serverId = 'office-bridge-test-server'

  // The panel is a LIVE refetch surface: opening it triggers the OfficeBridge
  // store's `GET /api/office-bridge/documents` (the notify-and-refetch source of
  // truth). On a desktop with Office this returns the open docs; on this headless
  // test backend the unsupported platform yields `[]`, which would empty the
  // panel. Mock the endpoint to return the same docs the tool reported, so the
  // panel reflects the live open-document state (as it would on a real desktop).
  await page.route('**/api/office-bridge/documents', route =>
    route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify(documents),
    }),
  )

  await mockChatTokenStream(page, [[startedEvent({ userMessageId }), completeEvent()]])

  const toolUse: MockMessageContent = {
    content_type: 'tool_use',
    content: {
      type: 'tool_use',
      id: toolUseId,
      name: 'list_open_documents',
      server_id: serverId,
      input: {},
    },
  }
  const toolResult: MockMessageContent = {
    content_type: 'tool_result',
    content: {
      type: 'tool_result',
      tool_use_id: toolUseId,
      name: 'list_open_documents',
      server_id: serverId,
      content: `${documents.length} open Office document(s).`,
      structured_content: { documents },
      is_error: false,
    },
  }

  await mockGetMessages(page, [
    mockUserMessage({ id: userMessageId, text: 'what documents are open?' }),
    { id: assistantMessageId, role: 'assistant', contents: [toolUse, toolResult] },
  ])

  await goToNewChatPage(page, baseURL)
  await selectModelInDropdown(page, 'GPT-4o Mini')
  const textarea = page.locator('textarea[placeholder*="Type your message"]').first()
  await textarea.fill('what documents are open?')
  await byTestId(page, 'chat-input-send-btn').click()

  await page
    .locator(`[data-testid="chat-message"][data-message-id="${assistantMessageId}"]`)
    .first()
    .waitFor({ state: 'visible', timeout: 15000 })
}

function sampleDocuments(): OpenDoc[] {
  return [
    {
      app: 'word',
      name: 'Q3-Report.docx',
      full_name: 'C:/Users/analyst/Q3-Report.docx',
      path: 'C:/Users/analyst',
      saved: true,
      active: true,
      attach_method: 'com_get_active_object',
    },
    {
      app: 'excel',
      name: 'Budget.xlsx',
      full_name: 'C:/Users/analyst/Budget.xlsx',
      path: 'C:/Users/analyst',
      saved: false,
      active: false,
      attach_method: 'accessible_object_from_window',
    },
  ]
}

test.describe('Office bridge — open documents panel', () => {
  test.describe.configure({ retries: 2 })

  test.beforeEach(async ({ page, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await page.evaluate(() =>
      JSON.parse(localStorage.getItem('auth-storage')!).state.token,
    )
    const providerId = await createProviderViaAPI(apiURL, token, 'OpenAI', 'openai')
    await assignProviderToAdministratorsGroup(apiURL, token, providerId)
    await createModelViaAPI(apiURL, token, providerId, undefined, undefined, 'openai')
  })

  test('the list_open_documents card opens the panel listing the documents', async ({
    page,
    testInfra,
  }) => {
    await seedOpenDocumentsResult(page, testInfra.baseURL, sampleDocuments())

    // The inline tool-result card renders with its "N open documents" summary.
    const card = byTestId(page, 'office-docs-tool-result-card')
    await expect(card).toBeVisible({ timeout: 15000 })
    await expect(byTestId(page, 'office-docs-tool-result-summary')).toContainText(
      '2 open documents',
    )

    // Clicking the card's button opens the right-panel...
    await page.getByRole('button', { name: /Open panel/ }).click()

    // ...which lists the enumerated documents grouped by app. Scope the
    // doc-name assertions to the panel (the card's preview also lists names).
    const panel = byTestId(page, 'office-docs-panel')
    await expect(panel).toBeVisible({ timeout: 10000 })
    await expect(panel.getByText('Q3-Report.docx')).toBeVisible()
    await expect(panel.getByText('Budget.xlsx')).toBeVisible()
    // Status tags render (an active/saved Word doc + an unsaved Excel doc).
    await expect(byTestId(page, 'office-doc-active-word-0')).toBeVisible()
    await expect(byTestId(page, 'office-doc-saved-excel-0')).toContainText('Unsaved')
  })
})
