import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  createModelViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'
import {
  mockChatTokenStream,
  startedEvent,
  completeEvent,
  mockGetMessages,
  mockUserMessage,
  type MockMessageContent,
} from '../helpers/sse-mock-helpers'
import { goToNewChatPage, selectModelInDropdown } from './helpers/chat-helpers'

/**
 * Chat-panel rendering of a `run_from_workspace` tool result — the card that
 * offers "Save to my workflows" + "Download .tar.gz" for an LLM-authored
 * workflow the user just ran. Mirrors web-search-result-rendering.spec's
 * seeded-tool_result approach (deterministic, no live LLM): the backend result
 * carries `structuredContent.workspace_dir`, which the card reads to enable the
 * graduation actions.
 */
test.describe('Chat - run_from_workspace result card', () => {
  async function seedRunResult(page: any, apiURL: string, baseURL: string, isError = false) {
    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)
    const providerId = await createProviderViaAPI(apiURL, adminToken, 'OpenAI', 'openai')
    await assignProviderToAdministratorsGroup(apiURL, adminToken, providerId)
    await createModelViaAPI(apiURL, adminToken, providerId, undefined, undefined, 'openai')

    const toolUseId = `tu_wf_${Math.random().toString(36).slice(2, 9)}`
    const assistantMessageId = `amsg_wf_${Math.random().toString(36).slice(2, 9)}`
    const userMessageId = `umsg_wf_${Math.random().toString(36).slice(2, 9)}`

    await mockChatTokenStream(page, [[startedEvent({ userMessageId }), completeEvent()]])

    const toolUse: MockMessageContent = {
      content_type: 'tool_use',
      content: {
        type: 'tool_use',
        id: toolUseId,
        name: 'run_from_workspace',
        server_id: 'workflow-mcp-test-server',
        input: { dir: 'flow' },
      },
    }
    const toolResult: MockMessageContent = {
      content_type: 'tool_result',
      content: {
        type: 'tool_result',
        tool_use_id: toolUseId,
        name: 'run_from_workspace',
        server_id: 'workflow-mcp-test-server',
        content: isError ? 'sandbox exit code 3: boom on stderr' : 'workflow completed',
        structured_content: isError
          ? { code: 'RUN_FAILED', error: 'sandbox exit code 3: boom on stderr', workspace_dir: 'flow', failed_step: { id: 'boom' } }
          : { workspace_dir: 'flow', outputs: { greeting: 'hello' } },
        is_error: isError,
      },
    }

    await mockGetMessages(page, [
      mockUserMessage({ id: userMessageId, text: 'run my workflow' }),
      { id: assistantMessageId, role: 'assistant', contents: [toolUse, toolResult] },
    ])

    await goToNewChatPage(page, baseURL)
    await selectModelInDropdown(page, 'GPT-4o Mini')
    const textarea = page.locator('textarea[placeholder*="Type your message"]').first()
    await textarea.fill('run my workflow')
    await page.getByRole('button', { name: 'Send message' }).click()

    const assistantMsg = page
      .locator(`[data-testid="chat-message"][data-message-id="${assistantMessageId}"]`)
      .first()
    await assistantMsg.waitFor({ state: 'visible', timeout: 15000 })
    return assistantMsg
  }

  test('a successful run shows Save + Download actions', async ({ page, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    const assistantMsg = await seedRunResult(page, apiURL, baseURL, false)

    await expect(
      assistantMsg.getByTestId('workflow-workspace-run-actions'),
    ).toBeVisible()
    await expect(assistantMsg.getByTestId('workflow-save-to-mine')).toBeVisible()
    await expect(assistantMsg.getByTestId('workflow-download-targz')).toBeVisible()
  })

  test('Save to my workflows calls workspace-save and reflects success', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    // Intercept the promote call → 201 with a permanent workflow.
    let saveHit = false
    await page.route('**/api/workflows/workspace-save', async (route) => {
      saveHit = true
      await route.fulfill({
        status: 201,
        contentType: 'application/json',
        body: JSON.stringify({ id: 'wf-saved-1', name: 'local.dev.<u>/saved', ephemeral: false }),
      })
    })
    const assistantMsg = await seedRunResult(page, apiURL, baseURL, false)

    await assistantMsg.getByTestId('workflow-save-to-mine').click()
    await expect.poll(() => saveHit, { timeout: 10000 }).toBe(true)
    // The button flips to the saved state.
    await expect(assistantMsg.getByTestId('workflow-save-to-mine')).toContainText(/Saved/i)
  })

  test('Download .tar.gz issues the export request', async ({ page, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    let exportHit = false
    await page.route('**/api/workflows/workspace-export**', async (route) => {
      exportHit = true
      await route.fulfill({
        status: 200,
        headers: {
          'content-type': 'application/gzip',
          'content-disposition': 'attachment; filename="flow.tar.gz"',
        },
        body: Buffer.from([0x1f, 0x8b, 0x08, 0x00]), // gzip magic
      })
    })
    const assistantMsg = await seedRunResult(page, apiURL, baseURL, false)

    await assistantMsg.getByTestId('workflow-download-targz').click()
    await expect.poll(() => exportHit, { timeout: 10000 }).toBe(true)
  })

  test('a failed run suppresses the graduation actions', async ({ page, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    const assistantMsg = await seedRunResult(page, apiURL, baseURL, true)
    // is_error → the card offers NO Save/Download (nothing worth graduating yet).
    // The failed step's stderr is surfaced by the tool-call bubble, not this card.
    await expect(
      assistantMsg.getByTestId('workflow-workspace-run-actions'),
    ).toHaveCount(0)
  })
})
