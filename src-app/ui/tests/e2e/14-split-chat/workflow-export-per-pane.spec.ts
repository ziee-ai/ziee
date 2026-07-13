import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import { mockGetMessages } from '../helpers/sse-mock-helpers'

/**
 * Split-chat E2E — the workflow "Download .tar.gz" / "Save to my workflows" card
 * exports the OWNING pane's conversation (TEST-100, audit #6). This leg was
 * CLAIMED by TEST-58 but never exercised (phantom). `WorkflowWorkspaceRunCard`
 * already binds `useChatPaneOrNull()?.store` (ITEM-38); this proves it by RUNNING:
 * a `run_from_workspace` tool-result card in pane B, then — with pane A focused —
 * clicking Download issues the export request with pane B's conversation_id, not
 * the focused pane A's. The tool result is mocked at the message boundary; the
 * pane→conversation routing (the crux) is real.
 */
test.describe('Split chat — per-pane workflow-workspace export', () => {
  test('downloading pane B\'s workflow card (pane A focused) exports pane B, not A', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    const auth = { Authorization: `Bearer ${token}` }
    const mkConv = async (t: string) =>
      (await (await page.request.post(`${apiURL}/api/conversations`, { headers: auth, data: { title: t } })).json()).id as string
    const convA = await mkConv('WF Alpha')
    const convB = await mkConv('WF Bravo')

    // A run_from_workspace tool-result message (with an authored workspace_dir) for
    // convB's reload → the graduate card renders in pane B.
    await mockGetMessages(page, [
      {
        id: 'amsg_wf',
        role: 'assistant',
        contents: [
          {
            id: 'c_wf',
            content_type: 'tool_result',
            content: {
              type: 'tool_result',
              tool_use_id: 'tu_wf',
              name: 'run_from_workspace',
              is_error: false,
              structured_content: { workspace_dir: 'my-wf-dir' },
              content: [],
            },
          },
        ],
      },
    ] as never)
    // Scope convA to EMPTY so pane A doesn't render the card from the shared mock.
    await page.route(new RegExp(`/api/conversations/${convA}/messages(\\?|$)`), async (route, req) => {
      if (req.method() !== 'GET') return route.fallback()
      await route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({ messages: [], has_more_before: false, has_more_after: false }) })
    })

    // Capture the export request's conversation_id and short-circuit the download.
    let exportedConversationId: string | null = null
    await page.route(/\/api\/workflows\/workspace-export/, async route => {
      exportedConversationId = new URL(route.request().url()).searchParams.get('conversation_id')
      await route.fulfill({ status: 200, contentType: 'application/gzip', body: 'dummy' })
    })

    // [A | B] split.
    await page.goto(`${baseURL}/chat/${convA}`)
    await page.waitForLoadState('load')
    await byTestId(page, 'chat-split-btn').click()
    const pane0 = byTestId(page, 'chat-pane-0')
    const pane1 = byTestId(page, 'chat-pane-1')
    await expect(pane1).toBeVisible({ timeout: 15000 })
    await pane1.getByTestId(`conversation-picker-item-${convB}`).click()

    // The graduate card renders in pane B; pane A has none.
    await expect(pane1.getByTestId('workflow-download-targz')).toBeVisible({ timeout: 15000 })
    await expect(pane0.getByTestId('workflow-download-targz')).toHaveCount(0)

    // Focus pane A, THEN download from pane B's card.
    await pane0.click()
    await expect(pane0).toHaveClass(/opacity-100/)
    await pane1.getByTestId('workflow-download-targz').click()

    // CRUX: the export targeted pane B's conversation, not the focused pane A's.
    await expect.poll(() => exportedConversationId, { timeout: 15000 }).toBe(convB)
    expect(exportedConversationId).not.toBe(convA)
  })
})
