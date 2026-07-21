import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getCurrentUserToken } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'
import { goToNewChatPage, selectModelInDropdown } from './helpers/chat-helpers'
import {
  createBridgeToolModel,
  updateAgentAdminSettings,
  HAS_BRIDGE,
  BRIDGE_SKIP,
} from './helpers/agent-llm-helpers'

/**
 * TEST-11 / ITEM-4 — the delegated **sub-agent activity card** renders inline in
 * the assistant turn, driven by the REAL agent-core chat loop + a real
 * tool-capable model (no mocks).
 *
 * With the admin `delegate_enabled` toggle on, the agent-core chat path offers
 * the core `delegate` tool (a fan-out to parallel sub-agents). The model is asked
 * to delegate two INDEPENDENT research sub-tasks in parallel. `AgentCore::fan_out`
 * spawns the children and emits an `AgentEvent::SubAgentActivity { run_id,
 * children }` at the START snapshot (children running/pending) and on each child's
 * terminal transition → the `subAgentActivity` chat SSE frame → the
 * `sub-agent-activity` chat extension keys the child list to the in-flight
 * assistant message → the committed `SubAgentActivityCard` (testid
 * `agent-subagents-card`) re-renders in place. We assert the card appears with ≥2
 * child rows and that at least one child transitions to a terminal `completed`
 * state (running → done), proving the card tracks per-child status live over SSE.
 *
 * Requires the agent-core chat path (ZIEE_CHAT_AGENT_CORE=1) + a real LLM bridge
 * (OPENAI_BASE_URL + OPENAI_API_KEY + ZIEE_TEST_LLM_MODEL). Skips cleanly when the
 * bridge env is unset. (The core-tool clobber fix — `mcp.rs` merging the core
 * `delegate`/`task_*` tools with MCP tools — is what makes `delegate` reach the
 * model; without it the model never sees the tool.)
 */
test.describe('agent sub-agent activity — delegated fan-out card (real LLM, agent-core)', () => {
  test.skip(!HAS_BRIDGE, BRIDGE_SKIP)
  test.setTimeout(240_000)

  test('a tool-capable model delegates 2 sub-agents in parallel → activity card shows per-child running→done', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getCurrentUserToken(page)

    // Deployment-wide `delegate_enabled` is what makes the agent-core chat path
    // OFFER the core `delegate` tool (ITEM-2 / DEC-2).
    await updateAgentAdminSettings(page, apiURL, token, { delegate_enabled: true })

    const providerId = await createProviderViaAPI(apiURL, token, 'OpenAI', 'openai')
    await assignProviderToAdministratorsGroup(apiURL, token, providerId)
    await createBridgeToolModel(page, apiURL, token, providerId, 'Delegate Agent Model')

    await goToNewChatPage(page, baseURL)
    await selectModelInDropdown(page, 'Delegate Agent Model')

    const textarea = page.locator('textarea[placeholder*="Type your message"]')
    await textarea.fill(
      'You have a `delegate` tool that fans out INDEPENDENT sub-tasks to fresh sub-agents ' +
        'that run in parallel. Call the `delegate` tool NOW with a `children` array of EXACTLY ' +
        'TWO items: the first child with system "Research photosynthesis and give a one-sentence ' +
        'summary."; the second child with system "Research mitosis and give a one-sentence ' +
        'summary.". You MUST call the delegate tool with both children in a single call — do NOT ' +
        'answer the two questions yourself.',
    )
    await page.getByRole('button', { name: 'Send message' }).click()

    // The delegated sub-agent activity card mounts inline in the assistant turn.
    const card = page.locator('[data-testid="agent-subagents-card"]').first()
    await expect(card).toBeVisible({ timeout: 210_000 })

    // ≥2 child rows land (the fan-out spawned two sub-agents).
    await expect
      .poll(async () => card.locator('[data-testid^="agent-subagents-card-child-"]').count(), {
        timeout: 60_000,
      })
      .toBeGreaterThanOrEqual(2)

    // At least one child reaches the terminal `completed` state — proves the card
    // tracks per-child running → done over the live SubAgentActivity SSE frames,
    // not a single static snapshot.
    await expect(
      card
        .locator('[data-testid^="agent-subagents-card-child-"][data-status="completed"]')
        .first(),
    ).toBeVisible({ timeout: 180_000 })
  })
})
