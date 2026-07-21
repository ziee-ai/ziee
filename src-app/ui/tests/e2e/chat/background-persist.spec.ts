import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getCurrentUserToken } from '../../common/auth-helpers'
import { byTestId } from '../testid'
import {
  seedBridgeConversation,
  spawnBackgroundSubagent,
  HAS_BRIDGE,
  BRIDGE_SKIP,
} from './helpers/agent-llm-helpers'

/**
 * TEST-25 / ITEM-10 — a background sub-agent run PERSISTS across a page reload
 * (snapshot-on-connect rehydrate), driven by a REAL detached sub-agent turn.
 *
 * A real background run is launched through the production `spawn_background`
 * path — the built-in `background_mcp` JSON-RPC endpoint that the chat model calls
 * — so a real bridge sub-agent turn actually executes on the `workflow_runs`
 * backbone. The `/background-tasks` page fetches the owner's runs through the real
 * `GET /api/background/runs` endpoint on mount, so the run's card survives a full
 * page reload: the durable `workflow_runs` row is the source of truth, not
 * transient in-memory state. We assert the run card is present, reload, and assert
 * it is STILL present (same run id) — the rehydrate.
 *
 * Requires the agent-core chat path + a real LLM bridge. Skips cleanly when unset.
 */
test.describe('background run — persists across reload (real sub-agent, agent-core)', () => {
  test.skip(!HAS_BRIDGE, BRIDGE_SKIP)
  test.setTimeout(120_000)

  test('a launched background sub-agent run survives a page reload', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getCurrentUserToken(page)

    const { conversationId } = await seedBridgeConversation(
      page,
      apiURL,
      token,
      'Background Persist Model',
    )

    // Launch a REAL detached sub-agent run on the durable backbone.
    const runId = await spawnBackgroundSubagent(
      page,
      apiURL,
      token,
      conversationId,
      'Reply with the single word DONE and nothing else.',
    )

    // The run surfaces on the background-tasks page (fetched from the real REST
    // endpoint on mount).
    await page.goto(`${baseURL}/background-tasks`)
    await expect(byTestId(page, 'background-tasks-page')).toBeVisible({ timeout: 30_000 })
    const card = byTestId(page, `background-run-card-${runId}`)
    await expect(card).toBeVisible({ timeout: 30_000 })
    await expect(byTestId(page, `background-run-kind-${runId}`)).toHaveText('Sub-agent')

    // Reload → the durable run row rehydrates through the same REST fetch; the
    // background task is NOT lost across the reload.
    await page.reload()
    await expect(byTestId(page, 'background-tasks-page')).toBeVisible({ timeout: 30_000 })
    await expect(byTestId(page, `background-run-card-${runId}`)).toBeVisible({ timeout: 30_000 })
    // The empty state is (still) gone — a real run persists.
    await expect(byTestId(page, 'background-tasks-empty')).toHaveCount(0)
  })
})
