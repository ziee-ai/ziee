import { mkdtempSync, writeFileSync } from 'fs'
import { tmpdir } from 'os'
import path from 'path'
import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  createModelViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'
import {
  goToNewChatPage,
  selectModelInDropdown,
  sendChatMessage,
} from './helpers/chat-helpers'
import { attachFileViaUI } from './helpers/file-panel-helpers'

/**
 * E2E — the files_mcp semantic_search tool used IN the chat panel. The
 * file-rag specs only cover the admin page; no E2E drives the chat-panel
 * MCP tool-call flow. semantic_search works day-one in FTS mode (no embedder),
 * so an attached file is searchable. Real-LLM gated.
 */

const HAS_ANTHROPIC = Boolean(process.env.ANTHROPIC_API_KEY)

test.describe('Chat — semantic_search files_mcp tool (real LLM)', () => {
  test.skip(!HAS_ANTHROPIC, 'ANTHROPIC_API_KEY not set — real-LLM semantic_search E2E skipped')

  test('the model calls semantic_search over an attached file and a result renders', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    const providerId = await createProviderViaAPI(apiURL, token, 'Anthropic', 'anthropic')
    await assignProviderToAdministratorsGroup(apiURL, token, providerId)
    await createModelViaAPI(
      apiURL,
      token,
      providerId,
      'claude-haiku-4-5-20251001',
      'Claude Haiku 4.5',
      'anthropic',
    )

    const dir = mkdtempSync(path.join(tmpdir(), 'ziee-sem-'))
    const file = path.join(dir, 'notes.txt')
    writeFileSync(
      file,
      'Photosynthesis converts light energy into chemical energy in the chloroplast. ' +
        'The Calvin cycle fixes carbon dioxide into glucose in the stroma.',
    )

    await goToNewChatPage(page, baseURL)
    await selectModelInDropdown(page, 'Claude Haiku 4.5')
    await attachFileViaUI(page, file)
    await sendChatMessage(
      page,
      'Use the semantic_search tool to find what happens in the chloroplast in my attached files, then summarize. You MUST call semantic_search.',
      false,
    )

    // The files_mcp tool result renders in the chat as a files tool-result card.
    await expect(page.locator('[data-testid="tool-result-files"]').first()).toBeVisible({
      timeout: 120_000,
    })
  })
})
