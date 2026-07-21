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
 * TEST-14 / ITEM-6 — friendly domain phrasing of a delegation, driven by the REAL
 * agent-core chat loop + a real tool-capable model.
 *
 * When the model fans out via the `delegate` tool, the timeline surface is the
 * `SubAgentActivityCard` — headed "Delegated sub-agents" with a friendly, human
 * per-child label (`subagent_label` = the first line of each child's system
 * instruction), NOT the internal `fan_out` / `spawn_subagents` jargon. This spec
 * asserts the domain phrasing: the friendly header + accessible name are present,
 * the two children render with their human-readable objective labels, and NO raw
 * jargon token (`fan_out`, `spawn_subagents`, `fan-out`) leaks into the visible
 * card.
 *
 * Requires the agent-core chat path (ZIEE_CHAT_AGENT_CORE=1) + a real LLM bridge.
 * Skips cleanly when the bridge env is unset.
 */
test.describe('agent sub-agent activity — friendly phrasing (real LLM, agent-core)', () => {
  test.skip(!HAS_BRIDGE, BRIDGE_SKIP)
  test.setTimeout(240_000)

  test('a delegated fan-out surfaces friendly domain phrasing, no fan_out jargon', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getCurrentUserToken(page)

    await updateAgentAdminSettings(page, apiURL, token, { delegate_enabled: true })

    const providerId = await createProviderViaAPI(apiURL, token, 'OpenAI', 'openai')
    await assignProviderToAdministratorsGroup(apiURL, token, providerId)
    await createBridgeToolModel(page, apiURL, token, providerId, 'Delegate Phrasing Model')

    await goToNewChatPage(page, baseURL)
    await selectModelInDropdown(page, 'Delegate Phrasing Model')

    const textarea = page.locator('textarea[placeholder*="Type your message"]')
    await textarea.fill(
      'You have a `delegate` tool that fans out INDEPENDENT sub-tasks to fresh sub-agents ' +
        'that run in parallel. Call the `delegate` tool NOW with a `children` array of EXACTLY ' +
        'TWO items: the first child with system "Summarize the water cycle in one sentence."; ' +
        'the second child with system "Summarize plate tectonics in one sentence.". You MUST ' +
        'call the delegate tool with both children in a single call — do NOT answer yourself.',
    )
    await page.getByRole('button', { name: 'Send message' }).click()

    // The activity card carries the friendly domain phrasing (header + a11y name).
    const card = page.locator('[data-testid="agent-subagents-card"]').first()
    await expect(card).toBeVisible({ timeout: 210_000 })
    await expect(card).toContainText('Delegated sub-agents')
    await expect(card).toHaveAttribute('aria-label', 'Delegated sub-agents')

    // Two children render, each with a human-readable objective label (the first
    // line of its system instruction) — not an opaque id or jargon.
    const children = card.locator('[data-testid^="agent-subagents-card-child-"]')
    await expect.poll(async () => children.count(), { timeout: 60_000 }).toBeGreaterThanOrEqual(2)
    const cardText = (await card.innerText()).toLowerCase()
    expect(cardText).toMatch(/water cycle|plate tectonics/)

    // NO internal fan-out jargon leaks into the user-facing card copy.
    expect(cardText).not.toContain('fan_out')
    expect(cardText).not.toContain('fan-out')
    expect(cardText).not.toContain('spawn_subagent')
  })
})
