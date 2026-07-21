import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { byTestId } from '../testid'

/**
 * TEST-46 (ITEM-13) — the Background tasks surface.
 *
 * asserts (TESTS.md): a long background command surfaces in the panel with live
 * output + a terminal dot; reopening rehydrates via snapshot-on-connect.
 *
 * The no-LLM half proven here (per the task's reframe): the `/background-tasks`
 * page renders its empty state, and once a background run exists it renders that
 * run's card — status + kind (Sub-agent) + label. A background run is a
 * `workflow_runs` row with `job_kind <> 'workflow'`; there is no create API (the
 * agent/sandbox backbone spawns them), so the row is seeded directly into the
 * per-test DB via `sql()` and the page's mount-time `loadRuns()` fetches it through
 * the real `GET /api/background/runs` endpoint.
 *
 * (The live-output stream + snapshot-on-connect rehydrate need a running sandbox
 * exec / LLM sub-agent and are reported separately.)
 */
test.describe('Background tasks page (ITEM-13)', () => {
  test('renders the empty state, then a seeded sub-agent run card', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, sql } = testInfra
    await loginAsAdmin(page, baseURL)

    // Empty state first (no background runs yet).
    await page.goto(`${baseURL}/background-tasks`)
    await expect(byTestId(page, 'background-tasks-page')).toBeVisible({
      timeout: 30000,
    })
    await expect(byTestId(page, 'background-tasks-empty')).toBeVisible({
      timeout: 15000,
    })

    // Seed a background sub-agent run (job_kind='subagent' → a background-backbone
    // row, never a classic workflow run). `inputs_json.task` becomes the card label.
    const adminId = (
      await sql(`SELECT id FROM users WHERE username = 'admin' LIMIT 1`)
    ).rows[0].id as string
    const label = 'Long-running background analysis'
    const inserted = await sql(
      `INSERT INTO workflow_runs (user_id, job_kind, status, inputs_json)
       VALUES ($1, 'subagent', 'running', $2::jsonb)
       RETURNING id`,
      [adminId, JSON.stringify({ task: label })],
    )
    const runId = inserted.rows[0].id as string

    // Reload → the page fetches the seeded run through the real REST endpoint.
    await page.goto(`${baseURL}/background-tasks`)
    await expect(byTestId(page, 'background-tasks-page')).toBeVisible({
      timeout: 30000,
    })
    const card = byTestId(page, `background-run-card-${runId}`)
    await expect(card).toBeVisible({ timeout: 15000 })
    // Status badge (running) + kind label (Sub-agent) + the run label.
    await expect(byTestId(page, `background-run-status-${runId}`)).toHaveText(
      'running',
    )
    await expect(byTestId(page, `background-run-kind-${runId}`)).toHaveText(
      'Sub-agent',
    )
    await expect(card).toContainText(label)
    // The empty state is gone once a run exists.
    await expect(byTestId(page, 'background-tasks-empty')).toHaveCount(0)
  })
})
