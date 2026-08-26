import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import { byTestId } from '../testid'
import {
  adminUserId,
  openTasksPanel,
  seedConversationWithMessage,
} from './helpers/background-helpers'

/**
 * TEST-2 / TEST-3 / TEST-4 (ITEM-3 / ITEM-5) — a completed background run's card
 * expands into a two-tab detail region whose DEFAULT, discoverable **Transcript**
 * tab renders the durable agent-loop transcript via the shared workflow
 * `AgentActivityTimeline` (`stepId="agent"`, testid `wf-activity-timeline-agent`),
 * and whose **Result** tab renders the final output. The transcript SURVIVES a
 * full page reload; a run that recorded NO agent activity shows the Transcript
 * tab's empty note while the Result tab still renders.
 *
 * This supersedes the pre-redesign flow where the timeline was buried below the
 * result under a single "View result" toggle with no "Transcript" affordance.
 *
 * SQL-seeding (rather than a real bridge sub-agent) is deliberate: it makes the
 * transcript deterministic (a fixed thinking / tool / message sequence) so the
 * spec always runs and asserts specific rows. Its `activity` lives in
 * `step_logs_json['agent::agent_activity']`, the exact projection the real
 * `GET /api/background/runs/{id}` endpoint reads. NO API mocking.
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

test.describe('background run — transcript tab (ITEM-3)', () => {
  test('TEST-2/TEST-3: a completed run opens the Transcript tab (shared timeline) + a Result tab, surviving reload', async ({
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

    // Detail is lazily fetched on expand: no timeline before the region opens.
    await expect(byTestId(page, 'wf-activity-timeline-agent')).toHaveCount(0)

    // Expand → the detail region shows a NAMED "Transcript" tab, selected by
    // default (TEST-3: discoverable, not buried under the result).
    await byTestId(page, `background-run-details-toggle-${runId}`).click()
    const transcriptTab = byTestId(page, `background-run-detail-tabs-${runId}-tab-transcript`)
    await expect(transcriptTab).toBeVisible({ timeout: 15_000 })
    await expect(transcriptTab).toContainText('Transcript')

    // TEST-3 (INV-3): drawn by the SHARED workflow timeline, one row per entry.
    const timeline = byTestId(page, 'wf-activity-timeline-agent')
    await expect(timeline).toBeVisible({ timeout: 15_000 })
    await expect(timeline.locator('[data-testid^="wf-activity-row-agent-"]')).toHaveCount(
      TRANSCRIPT.length,
    )

    // The Result tab renders the existing BackgroundRunResult final-text (reuse).
    await byTestId(page, `background-run-detail-tabs-${runId}-tab-result`).click()
    await expect(byTestId(page, `background-run-final-text-${runId}`)).toBeVisible({
      timeout: 15_000,
    })

    // TEST-2 durable rehydrate: reload → reopen the persisted Tasks tab →
    // re-expand → the transcript is STILL served through the same REST endpoint.
    await page.reload()
    await expect(byTestId(page, 'background-panel-list')).toBeVisible({ timeout: 30_000 })
    await expect(card).toBeVisible({ timeout: 15_000 })
    await byTestId(page, `background-run-details-toggle-${runId}`).click()
    await expect(byTestId(page, 'wf-activity-timeline-agent')).toBeVisible({ timeout: 15_000 })
    await expect(
      byTestId(page, 'wf-activity-timeline-agent').locator(
        '[data-testid^="wf-activity-row-agent-"]',
      ),
    ).toHaveCount(TRANSCRIPT.length)
  })

  test('TEST-4: a run with no recorded activity shows the empty note; Result tab still renders', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL, sql } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    const userId = await adminUserId(sql)

    const conv = await seedConversationWithMessage(page, apiURL, token, sql, 'Quiet transcript chat')
    const runId = await seedCompletedRun(sql, userId, conv, { task: 'A run with no transcript' })

    await openTasksPanel(page, baseURL, conv)
    await expect(byTestId(page, `background-run-card-${runId}`)).toBeVisible({ timeout: 15_000 })

    await byTestId(page, `background-run-details-toggle-${runId}`).click()

    // The default Transcript tab shows the friendly empty note (never a blank
    // tab or a timeline) for an activity-less run.
    await expect(byTestId(page, `background-run-transcript-empty-${runId}`)).toBeVisible({
      timeout: 15_000,
    })
    await expect(byTestId(page, 'wf-activity-timeline-agent')).toHaveCount(0)

    // Positive control: the detail DID load — the Result tab renders the body.
    await byTestId(page, `background-run-detail-tabs-${runId}-tab-result`).click()
    await expect(byTestId(page, `background-run-final-text-${runId}`)).toBeVisible({
      timeout: 15_000,
    })
  })
})
