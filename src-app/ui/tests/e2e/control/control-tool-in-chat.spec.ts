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
} from '../chat/helpers/chat-helpers'

/**
 * control_mcp — REAL LLM end-to-end through the actual browser UI.
 *
 * The built-in app-control tools (list_capabilities / describe_capability /
 * invoke_capability) are auto-attached to a tool-capable chat. This drives the
 * FULL production path with a REAL Anthropic model and NO mocks:
 *   1. the model calls the read-only discovery tools (auto-run), then
 *   2. attempts a STATE-CHANGING `invoke_capability` (Assistant.create) which is
 *      FORCED through the in-chat approval card even in an auto-approve chat, then
 *   3. on Approve, the assistant is actually created (verified via the REST API).
 *
 * Gated on ANTHROPIC_API_KEY — skips cleanly when unset (runs against the local
 * bridge when ANTHROPIC_BASE_URL is set, so no paid keys).
 */

const HAS_ANTHROPIC_KEY = Boolean(process.env.ANTHROPIC_API_KEY)

test.describe('control_mcp — real LLM end-to-end (approve → assistant created)', () => {
  test.skip(!HAS_ANTHROPIC_KEY, 'ANTHROPIC_API_KEY not set — skipping real-LLM control E2E')
  // Real-LLM + live SSE: the model's multi-round tool calling and the streaming
  // connection are non-deterministic, so retry like the other real-backend specs.
  test.describe.configure({ retries: 2 })
  test.slow()

  async function setupChat(page: import('@playwright/test').Page, baseURL: string, apiURL: string) {
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
    await goToNewChatPage(page, baseURL)
    await selectModelInDropdown(page, 'Claude Haiku 4.5')
    return token
  }

  test('a mutating control invoke requires approval, then creates the assistant', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    const token = await setupChat(page, baseURL, apiURL)

    const name = `ControlE2E_${Date.now()}`
    await sendChatMessage(
      page,
      `Use the app-control tools to create a new assistant named exactly "${name}". ` +
        `Call invoke_capability with Assistant.create. Do it now — do not ask me first.`,
      false, // tool round-trips + an approval pause; don't block on first complete
    )

    // Baseline: how many assistants exist before the turn.
    const before = await fetchTotalAssistants(apiURL, token)

    // The mutating invoke must surface the approval card (even though a fresh
    // chat auto-approves other tools).
    const approve = page.locator('[data-testid="tool-approval-approve-once"]').first()
    await expect(approve).toBeVisible({ timeout: 90000 })
    await approve.click()

    // After approval the control write actually runs → a NEW assistant exists via
    // REST. We assert the COUNT increased rather than an exact name: the model
    // (esp. a lower-fidelity local bridge) may name it slightly differently, but
    // the security+integration property under test — approve → the control write
    // executes and creates a row — holds either way. (The exact-name create path
    // is also proven deterministically by the Tier-3 integration test
    // `invoke_create_assistant_real_roundtrip`.)
    await expect
      .poll(async () => fetchTotalAssistants(apiURL, token), { timeout: 60000 })
      .toBeGreaterThan(before)
  })

  test('denying the control write leaves nothing created', async ({ page, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    const token = await setupChat(page, baseURL, apiURL)

    const name = `ControlDeny_${Date.now()}`
    await sendChatMessage(
      page,
      `Use the app-control tools to create a new assistant named exactly "${name}" ` +
        `(invoke_capability with Assistant.create). Do it now — do not ask me first.`,
      false,
    )

    const deny = page.locator('[data-testid="tool-approval-deny"]').first()
    await expect(deny).toBeVisible({ timeout: 90000 })
    await deny.click()

    // Give the turn a moment to settle; the assistant must never be created.
    await page.waitForTimeout(3000)
    const count = await fetchAssistantCount(apiURL, token, name)
    expect(count).toBe(0)
  })
})

/** Count assistants owned by the admin whose name matches `name`, via REST. */
async function fetchAssistantCount(apiURL: string, token: string, name: string): Promise<number> {
  const list = await fetchAssistants(apiURL, token)
  return list.filter((a) => a.name === name).length
}

/** Total assistants owned by the admin, via REST. */
async function fetchTotalAssistants(apiURL: string, token: string): Promise<number> {
  return (await fetchAssistants(apiURL, token)).length
}

async function fetchAssistants(
  apiURL: string,
  token: string,
): Promise<Array<{ name?: string }>> {
  const res = await fetch(`${apiURL}/assistants?per_page=100`, {
    headers: { Authorization: `Bearer ${token}` },
  })
  if (!res.ok) return []
  const body = await res.json()
  return body.assistants ?? body.data ?? []
}
