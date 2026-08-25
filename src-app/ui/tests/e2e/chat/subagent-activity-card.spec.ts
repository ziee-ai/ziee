import type { Locator, Page } from '@playwright/test'
import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getCurrentUserToken } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'
import { adminUserId } from '../15-background/helpers/background-helpers'
import { goToNewChatPage, selectModelInDropdown } from './helpers/chat-helpers'
import {
  createBridgeToolModel,
  updateAgentAdminSettings,
  HAS_BRIDGE,
  BRIDGE_SKIP,
} from './helpers/agent-llm-helpers'

/**
 * Drive a real delegated fan-out (the shared setup of every test below): enable
 * `delegate_enabled`, create a tool-capable bridge model, and ask the model to
 * fan out EXACTLY TWO independent research sub-agents. Returns the (live,
 * SSE-fed) `SubAgentActivityCard` locator once it has mounted with ≥2 children.
 */
async function driveFanOut(
  page: Page,
  apiURL: string,
  baseURL: string,
  token: string,
): Promise<Locator> {
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

  const card = page.locator('[data-testid="agent-subagents-card"]').first()
  await expect(card).toBeVisible({ timeout: 210_000 })
  await expect
    .poll(async () => card.locator('[data-testid^="agent-subagents-card-child-"]').count(), {
      timeout: 60_000,
    })
    .toBeGreaterThanOrEqual(2)
  return card
}

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

  /**
   * TEST-13 / ITEM-9 — a fan-out child row DRILLS IN to its full agent-loop
   * transcript. Clicking a completed child fetches `GET /api/subagent-runs/{id}`
   * and renders it inline via the shared workflow `AgentActivityTimeline` (NOT a
   * drawer). The transcript's SOURCE is durable across reload — verified through
   * the endpoint, because the live SSE card itself is ephemeral by design (the
   * backend never replays `subAgentActivity`, so a reloaded, finished fan-out has
   * no card until a fresh frame; ITEM-9's durability is the on-demand REST
   * re-fetch of each child's transcript, which is what survives).
   */
  test('a completed child row expands to its transcript timeline; the transcript source survives reload', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getCurrentUserToken(page)

    const card = await driveFanOut(page, apiURL, baseURL, token)

    // Wait for a child to finish, then drill into it.
    const completedLi = card
      .locator('[data-testid^="agent-subagents-card-child-"][data-status="completed"]')
      .first()
    await expect(completedLi).toBeVisible({ timeout: 180_000 })

    await completedLi
      .locator('[data-testid^="agent-subagents-card-child-toggle-"]')
      .click()

    // The drill-in renders the shared workflow timeline INLINE inside the row.
    const timeline = completedLi.locator('[data-testid^="wf-activity-timeline-"]')
    await expect(timeline).toBeVisible({ timeout: 60_000 })
    await expect(
      timeline.locator('[data-testid^="wf-activity-row-"]').first(),
    ).toBeVisible({ timeout: 60_000 })

    // The timeline's testid is `wf-activity-timeline-<childRunId>` — recover the
    // child run id to hit its durable transcript endpoint directly.
    const tid = await timeline.getAttribute('data-testid')
    const childId = (tid ?? '').replace('wf-activity-timeline-', '')
    expect(childId).not.toEqual('')

    const before = await page.request.get(`${apiURL}/api/subagent-runs/${childId}`, {
      headers: { Authorization: `Bearer ${token}` },
    })
    expect(before.status()).toBe(200)
    const beforeBody = await before.json()
    expect(Array.isArray(beforeBody.activity)).toBeTruthy()
    expect(beforeBody.activity.length).toBeGreaterThanOrEqual(1)

    // The durable transcript is still served AFTER a full page reload (the
    // `workflow_runs` row is the source of truth; the drill-in re-fetches it on
    // re-expand). The live card is SSE-ephemeral and intentionally not asserted
    // to re-appear here.
    await page.reload()
    const after = await page.request.get(`${apiURL}/api/subagent-runs/${childId}`, {
      headers: { Authorization: `Bearer ${token}` },
    })
    expect(after.status()).toBe(200)
    const afterBody = await after.json()
    expect(afterBody.activity.length).toBeGreaterThanOrEqual(1)
  })

  /**
   * TEST-13 / ITEM-9 — the mandatory entity-lifecycle case: a child whose durable
   * run has been PRUNED (retention / conversation delete) must render status-only
   * and NEVER crash the card. We delete the children's `workflow_runs` rows, then
   * expand a never-opened child: `GET /api/subagent-runs/{id}` 404s → the store
   * records `{status:'error'}` → the row shows a graceful "no longer available"
   * line while the live status badge (from SSE) and the whole card stay intact.
   */
  test('a pruned child (404) renders status-only without crashing the card', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL, sql } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getCurrentUserToken(page)
    const userId = await adminUserId(sql)

    const card = await driveFanOut(page, apiURL, baseURL, token)

    // Wait for a completed child so the run is terminal before we prune it.
    const completedLi = card
      .locator('[data-testid^="agent-subagents-card-child-"][data-status="completed"]')
      .first()
    await expect(completedLi).toBeVisible({ timeout: 180_000 })

    // Prune the durable transcripts (as retention / a conversation delete would).
    await sql(`DELETE FROM workflow_runs WHERE user_id = $1 AND job_kind = 'subagent'`, [
      userId,
    ])

    // Expand the (never-opened) completed child → its transcript 404s.
    await completedLi
      .locator('[data-testid^="agent-subagents-card-child-toggle-"]')
      .click()

    // Status-only: a graceful "no longer available" line, NO timeline, and the
    // card is still standing (no crash / no ErrorBoundary).
    await expect(completedLi.locator('[data-testid$="-error"]')).toBeVisible({
      timeout: 30_000,
    })
    await expect(completedLi.locator('[data-testid^="wf-activity-timeline-"]')).toHaveCount(0)
    await expect(card).toBeVisible()
    // The live status badge survives the prune (it is SSE-fed, not transcript-fed).
    await expect(completedLi).toHaveAttribute('data-status', 'completed')
  })
})
