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
 * code_sandbox — REAL LLM + REAL sandbox end-to-end through the browser UI.
 *
 * audit id d7b1a46a07b2 — real LLM + sandbox MCP execution was never tested via
 * the chat UI (only backend Tier-5 + a mock-MCP plot spec existed). This drives
 * the FULL production path with NO mocks: a real Anthropic model decides to call
 * the auto-attached code_sandbox `execute_command` tool, the command runs inside
 * the bwrap-isolated rootfs, the tool result renders in the chat, and the
 * assistant echoes a unique marker that ONLY real execution could produce.
 *
 * Gated on:
 *   - ANTHROPIC_API_KEY  — the real model.
 *   - ZIEE_E2E_SANDBOX=1 — boots the test server with code_sandbox enabled
 *     (see fixtures/test-context.ts); also requires a mounted rootfs on the
 *     host (ZIEE_SANDBOX_ROOTFS / the runtime auto-fetch path).
 * Skips cleanly when either is unset.
 */

const HAS_ANTHROPIC_KEY = Boolean(process.env.ANTHROPIC_API_KEY)
const SANDBOX_ENABLED = process.env.ZIEE_E2E_SANDBOX === '1'

test.describe('code_sandbox — real LLM end-to-end (execute_command via chat UI)', () => {
  test.skip(
    !(HAS_ANTHROPIC_KEY && SANDBOX_ENABLED),
    'requires ANTHROPIC_API_KEY + ZIEE_E2E_SANDBOX=1 (+ mounted rootfs)',
  )
  test.slow()

  test('real model runs a command in the sandbox and echoes the output', async ({
    page,
    testInfra,
  }) => {
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

    // A unique marker the model cannot know without actually running the command.
    const marker = `ZIEE_SANDBOX_MARKER_${Date.now()}`
    await sendChatMessage(
      page,
      `Use the code_sandbox execute_command tool to run exactly: echo ${marker}. ` +
        `Then tell me the exact stdout you got back. You MUST call the tool — do ` +
        `not answer from memory.`,
      false, // tool round-trip; don't block on the first complete event
    )

    // A tool result renders in the chat (the sandbox execute_command result).
    await expect(
      page.locator('[data-testid="chat-message"]').last(),
    ).toBeVisible({ timeout: 60000 })

    // The assistant ultimately echoes the marker that only real execution
    // (echo $marker inside the sandbox) could have produced.
    await expect
      .poll(
        async () =>
          (await page
            .locator('[data-role="assistant"]')
            .last()
            .textContent()) ?? '',
        { timeout: 120000 },
      )
      .toContain(marker)
  })
})

async function getAdminToken(
  page: import('@playwright/test').Page,
): Promise<string> {
  const authData = await page.evaluate(() =>
    localStorage.getItem('auth-storage'),
  )
  return JSON.parse(authData!).state.token
}
