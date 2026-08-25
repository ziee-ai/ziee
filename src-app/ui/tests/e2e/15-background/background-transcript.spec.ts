import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import { byTestId } from '../testid'
import {
  adminUserId,
  openTasksPanel,
  seedConversationWithMessage,
} from './helpers/background-helpers'

/**
 * TEST-12 / ITEM-4 — a completed background run's card lazily EXPANDS to render
 * its durable agent-loop transcript via the shared workflow `AgentActivityTimeline`
 * (`stepId="agent"`, testid `wf-activity-timeline-agent`), and that transcript
 * SURVIVES a full page reload; a run that recorded NO agent activity shows no
 * timeline block at all.
 *
 * Mirrors `background-persist.spec.ts` (durable rehydrate on reload) and the
 * SQL-seeding technique of `background-in-conversation.spec.ts`: there is no
 * create API for these runs (the agent backbone spawns them), so a `workflow_runs`
 * row is inserted directly — its `activity` transcript lives in
 * `step_logs_json['agent::agent_activity']`, the exact projection the real
 * `GET /api/background/runs/{id}` endpoint reads — and then exercised end-to-end
 * through the real REST fetch + the real card render. NO API mocking.
 *
 * SQL-seeding (rather than a real bridge sub-agent) is deliberate: it makes the
 * transcript deterministic (a fixed thinking / tool / message sequence) so the
 * spec always runs and asserts specific rows, instead of gating on a live LLM
 * whose minimal reply may record only one entry.
 */

/** The agent-activity transcript entries, in the persisted `ProgressKind::AgentActivity`
 *  wire shape (internally-tagged `type:"agent_activity"`, snake_case kind/status). */
const TRANSCRIPT = [
  { type: 'agent_activity', seq: 0, kind: 'thinking', title: 'Planning the approach', status: 'ok' },
  {
    type: 'agent_activity',
    seq: 1,
    kind: 'tool_call',
    tool: 'web_search',
    title: 'Searching the web',
    detail: 'query=photosynthesis',
    status: 'ok',
  },
  { type: 'agent_activity', seq: 2, kind: 'message', title: 'Wrote the final answer', status: 'ok' },
]

type Sql = (
  text: string,
  params?: unknown[],
) => Promise<{ rows: Record<string, unknown>[] }>

/**
 * Seed a COMPLETED background sub-agent run directly. When `activity` is given it
 * is stored under `step_logs_json['agent::agent_activity']` (the projection the
 * detail endpoint reads); omitting it leaves the default `{}` → an empty transcript.
 */
async function seedCompletedRun(
  sql: Sql,
  userId: string,
  conversationId: string,
  opts: { task: string; activity?: unknown[] },
): Promise<string> {
  const finalOutput = JSON.stringify({ final_text: 'DONE', tokens_used: 3 })
  if (opts.activity) {
    const stepLogs = JSON.stringify({ 'agent::agent_activity': opts.activity })
    const r = await sql(
      `INSERT INTO workflow_runs
         (user_id, job_kind, status, inputs_json, conversation_id, final_output_json, step_logs_json)
       VALUES ($1,'subagent','completed',$2::jsonb,$3,$4::jsonb,$5::jsonb)
       RETURNING id`,
      [userId, JSON.stringify({ task: opts.task }), conversationId, finalOutput, stepLogs],
    )
    return r.rows[0].id as string
  }
  const r = await sql(
    `INSERT INTO workflow_runs
       (user_id, job_kind, status, inputs_json, conversation_id, final_output_json)
     VALUES ($1,'subagent','completed',$2::jsonb,$3,$4::jsonb)
     RETURNING id`,
    [userId, JSON.stringify({ task: opts.task }), conversationId, finalOutput],
  )
  return r.rows[0].id as string
}

test.describe('background run — transcript drill-in (ITEM-4)', () => {
  test('a completed run expands to its AgentActivityTimeline and it survives a reload', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL, sql } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    const userId = await adminUserId(sql)

    const conv = await seedConversationWithMessage(page, apiURL, token, sql, 'Transcript chat')
    const runId = await seedCompletedRun(sql, userId, conv, {
      task: 'Summarise photosynthesis',
      activity: TRANSCRIPT,
    })

    await openTasksPanel(page, baseURL, conv)
    const card = byTestId(page, `background-run-card-${runId}`)
    await expect(card).toBeVisible({ timeout: 15_000 })

    // The transcript is lazily fetched on expand (like every other run detail):
    // no timeline before the result view is opened.
    await expect(byTestId(page, 'wf-activity-timeline-agent')).toHaveCount(0)

    // Expand the result → the durable transcript renders via the shared workflow
    // timeline, one row per recorded thinking / tool / message entry.
    await byTestId(page, `background-run-result-toggle-${runId}`).click()
    const timeline = byTestId(page, 'wf-activity-timeline-agent')
    await expect(timeline).toBeVisible({ timeout: 15_000 })
    await expect(timeline.locator('[data-testid^="wf-activity-row-agent-"]')).toHaveCount(
      TRANSCRIPT.length,
    )

    // Durable rehydrate: reload → reopen the persisted Tasks tab → re-expand →
    // the transcript is STILL served (the `workflow_runs` row is the source of
    // truth, re-fetched through the same REST endpoint), not lost across reload.
    await page.reload()
    await expect(byTestId(page, 'background-panel-list')).toBeVisible({ timeout: 30_000 })
    await expect(card).toBeVisible({ timeout: 15_000 })
    await byTestId(page, `background-run-result-toggle-${runId}`).click()
    await expect(byTestId(page, 'wf-activity-timeline-agent')).toBeVisible({ timeout: 15_000 })
    await expect(
      byTestId(page, 'wf-activity-timeline-agent').locator(
        '[data-testid^="wf-activity-row-agent-"]',
      ),
    ).toHaveCount(TRANSCRIPT.length)
  })

  test('a run with no recorded activity shows no timeline block', async ({ page, testInfra }) => {
    const { baseURL, apiURL, sql } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    const userId = await adminUserId(sql)

    const conv = await seedConversationWithMessage(page, apiURL, token, sql, 'Quiet transcript chat')
    const runId = await seedCompletedRun(sql, userId, conv, { task: 'A run with no transcript' })

    await openTasksPanel(page, baseURL, conv)
    await expect(byTestId(page, `background-run-card-${runId}`)).toBeVisible({ timeout: 15_000 })

    // Expand the result → the result body renders (positive control that the
    // detail loaded), but there is NO transcript timeline for an activity-less run.
    await byTestId(page, `background-run-result-toggle-${runId}`).click()
    await expect(byTestId(page, `background-run-result-panel-${runId}`)).toBeVisible({
      timeout: 15_000,
    })
    await expect(byTestId(page, `background-run-final-text-${runId}`)).toBeVisible()
    await expect(byTestId(page, 'wf-activity-timeline-agent')).toHaveCount(0)
  })
})
