import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
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

/**
 * files_mcp — REAL LLM end-to-end through the actual browser UI.
 *
 * The built-in files_mcp tools (write_file / read_file) are auto-attached to a
 * tool-capable chat. This drives the FULL production path with a REAL Anthropic
 * model and NO mocks: the model decides to call write_file to persist a unique
 * marker, the tool result renders in the chat, then read_file (or its own
 * memory of the write) lets the assistant echo the marker back — proving the
 * files_mcp tools are reachable and round-trip through the chat UI.
 *
 * Gated on ANTHROPIC_API_KEY — skips cleanly when unset.
 */

const HAS_ANTHROPIC_KEY = Boolean(process.env.ANTHROPIC_API_KEY)

test.describe('files_mcp — real LLM end-to-end (write_file → read_file round-trip)', () => {
  test.skip(!HAS_ANTHROPIC_KEY, 'ANTHROPIC_API_KEY not set — skipping real-LLM files_mcp E2E')
  test.slow()

  test('real model writes then reads a file through the chat UI', async ({ page, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(page)
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

    await goToNewChatPage(page, baseURL)
    await selectModelInDropdown(page, 'Claude Haiku 4.5')

    // A unique marker so the echo can't be a coincidence / hallucination.
    const marker = `ZIEE_FILE_MARKER_${Date.now()}`
    await sendChatMessage(
      page,
      `Use the write_file tool to create a file named note.txt whose entire contents are exactly: ` +
        `${marker}. Then use the read_file tool to read note.txt back and tell me the exact ` +
        `contents you read. You MUST call the file tools — do not answer from memory.`,
      false, // tool round-trips; don't block on the first complete event
    )

    // The files_mcp tool result renders as a tool-result-files card in the chat.
    await expect(page.locator('[data-testid="tool-result-files"]').first()).toBeVisible({
      timeout: 60000,
    })

    // …and the assistant ultimately echoes the round-tripped marker.
    await expect
      .poll(
        async () =>
          (await page.locator('[data-role="assistant"]').last().textContent()) ?? '',
        { timeout: 90000 },
      )
      .toContain(marker)
  })
})

async function getAdminToken(page: import('@playwright/test').Page): Promise<string> {
  const authData = await page.evaluate(() => localStorage.getItem('auth-storage'))
  return JSON.parse(authData!).state.token
}
