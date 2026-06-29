import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  createModelViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'
import {
  selectModelInDropdown,
  sendChatMessage,
} from '../chat/helpers/chat-helpers'

/**
 * Document-RAG `semantic_search` MCP tool — REAL LLM end-to-end through the
 * chat UI. The built-in files_mcp `semantic_search` tool is auto-attached to a
 * tool-capable chat and searches the conversation's project knowledge files.
 *
 * Setup (via API): an Anthropic model + a project whose knowledge file holds a
 * UNIQUE marker fact + a conversation bound to that project. We then ask a
 * question whose answer lives ONLY in the indexed file, so the model must call
 * `semantic_search` to retrieve it. The tool-result card + the marker echo
 * prove the chat-panel semantic_search flow works end-to-end.
 *
 * Gated on ANTHROPIC_API_KEY — skips cleanly when unset.
 */

const HAS_ANTHROPIC_KEY = Boolean(process.env.ANTHROPIC_API_KEY)

async function getAdminToken(
  page: import('@playwright/test').Page,
): Promise<string> {
  const authData = await page.evaluate(() =>
    localStorage.getItem('auth-storage'),
  )
  return JSON.parse(authData!).state.token
}

test.describe('file_rag — semantic_search via the chat panel (real LLM)', () => {
  test.skip(
    !HAS_ANTHROPIC_KEY,
    'ANTHROPIC_API_KEY not set — skipping real-LLM semantic_search E2E',
  )
  test.slow()

  test('model calls semantic_search to answer from a project knowledge file', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(page)
    const auth = { Authorization: `Bearer ${token}` }

    const providerId = await createProviderViaAPI(
      apiURL,
      token,
      'Anthropic',
      'anthropic',
    )
    await assignProviderToAdministratorsGroup(apiURL, token, providerId)
    await createModelViaAPI(
      apiURL,
      token,
      providerId,
      'claude-haiku-4-5-20251001',
      'Claude Haiku 4.5',
      'anthropic',
    )

    // A unique marker so the answer can't be a hallucination.
    const marker = `ZIEE_RAG_MARKER_${Date.now()}`
    const knowledge = `Internal facts document.\nThe secret project codeword is ${marker}.\n`

    // Project + knowledge file (upload+attach) + conversation bound to it.
    const project = await page.request
      .post(`${apiURL}/projects`, { headers: auth, data: { name: 'RAG Project' } })
      .then(r => r.json())
    const projectId: string = project.id

    await page.request.post(`${apiURL}/projects/${projectId}/files`, {
      headers: auth,
      multipart: {
        file: {
          name: 'facts.txt',
          mimeType: 'text/plain',
          buffer: Buffer.from(knowledge),
        },
      },
    })

    const conv = await page.request
      .post(`${apiURL}/conversations`, { headers: auth, data: {} })
      .then(r => r.json())
    const conversationId: string = conv.id
    await page.request.post(
      `${apiURL}/projects/${projectId}/conversations/${conversationId}`,
      { headers: auth },
    )

    // Give the background file_rag ingest time to produce searchable chunks.
    await page.waitForTimeout(6000)

    await page.goto(`${baseURL}/projects/${projectId}/chat/${conversationId}`)
    await selectModelInDropdown(page, 'Claude Haiku 4.5')

    await sendChatMessage(
      page,
      'Use the semantic_search tool to look up the secret project codeword in ' +
        'my project knowledge files, then tell me the exact codeword. You MUST ' +
        'call semantic_search — do not answer from memory.',
      false,
    )

    // The assistant ultimately echoes the marker retrieved via semantic_search.
    await expect
      .poll(
        async () =>
          (await page.locator('[data-role="assistant"]').last().textContent()) ??
          '',
        { timeout: 90000 },
      )
      .toContain(marker)
  })
})
