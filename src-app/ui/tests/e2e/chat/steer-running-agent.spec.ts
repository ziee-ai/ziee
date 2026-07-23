import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { byTestId } from '../testid'

/**
 * TEST-126 / ITEM-25 — steer a RUNNING background agent: a right-panel nudge is
 * accepted and the run CONTINUES (it is not restarted / killed), and the steering
 * panel rehydrates the queued note when reopened.
 *
 * The steer surface is the inline composer on each running `BackgroundRunCard`
 * (`/background-tasks`). Posting a note drives the REAL
 * `POST /api/background/runs/{id}/notes` endpoint, which enqueues a durable
 * steering note into the `background_run_notes` queue (the detached sub-agent
 * consumes it at its next iteration boundary) WITHOUT touching the run's lifecycle
 * — no cancel, no new run row. A background run is a `workflow_runs` row with
 * `job_kind='subagent'`; there is no create API (the agent backbone spawns them),
 * so a RUNNING row is seeded directly into the per-test DB, then the real UI +
 * endpoint + durable queue are exercised end-to-end. (Note delivery reflected on
 * the live sub-agent's next turn is covered by the integration steer test.)
 *
 * asserts: nudge accepted (201 + the pending note row rendered), the run stays
 * non-terminal (running — not restarted), and the panel rehydrates the note on
 * reopen (loadNotes refetch).
 */
test.describe('steer a running background agent (ITEM-25)', () => {
  test('a steering note is accepted, the run continues, and the panel rehydrates on reopen', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, sql } = testInfra
    await loginAsAdmin(page, baseURL)

    // Seed a RUNNING background sub-agent run owned by the admin.
    const adminId = (await sql(`SELECT id FROM users WHERE username = 'admin' LIMIT 1`))
      .rows[0].id as string
    const label = 'Long-running research sub-agent'
    const inserted = await sql(
      `INSERT INTO workflow_runs (user_id, job_kind, status, inputs_json)
       VALUES ($1, 'subagent', 'running', $2::jsonb)
       RETURNING id`,
      [adminId, JSON.stringify({ task: label })],
    )
    const runId = inserted.rows[0].id as string

    await page.goto(`${baseURL}/background-tasks`)
    await expect(byTestId(page, 'background-tasks-page')).toBeVisible({ timeout: 30_000 })
    const card = byTestId(page, `background-run-card-${runId}`)
    await expect(card).toBeVisible({ timeout: 30_000 })
    // The run is running before we steer it.
    await expect(byTestId(page, `background-run-status-${runId}`)).toHaveText('running')

    // Open the steer composer (only present on a non-terminal run).
    await byTestId(page, `background-run-steer-toggle-${runId}`).click()
    await expect(byTestId(page, `background-run-steer-${runId}`)).toBeVisible({ timeout: 10_000 })

    // Post a steering note → the REAL POST /api/background/runs/{id}/notes.
    const noteText = 'Also cover the safety considerations before you finish.'
    await byTestId(page, `background-run-note-input-${runId}`).fill(noteText)
    const [postResp] = await Promise.all([
      page.waitForResponse(
        r =>
          /\/api\/background\/runs\/[0-9a-f-]+\/notes$/.test(r.url()) &&
          r.request().method() === 'POST',
        { timeout: 15_000 },
      ),
      byTestId(page, `background-run-note-send-${runId}`).click(),
    ])
    expect(postResp.status(), `post note should 201: ${await postResp.text()}`).toBe(201)

    // Accepted: the queued pending note renders (loadNotes refetch after send).
    const pendingNote = page.locator(`[data-testid^="background-run-note-"]`).filter({
      hasText: noteText,
    })
    await expect(pendingNote.first()).toBeVisible({ timeout: 15_000 })

    // The run CONTINUES — not restarted / cancelled: still the same run, still running.
    await expect(byTestId(page, `background-run-status-${runId}`)).toHaveText('running')
    // Exactly one background run row exists (no restart spawned a second run).
    const countRows = await sql(
      `SELECT count(*)::int AS n FROM workflow_runs WHERE user_id = $1 AND job_kind = 'subagent'`,
      [adminId],
    )
    expect(countRows.rows[0].n).toBe(1)

    // Panel rehydrates on reopen: close the composer, reopen → loadNotes surfaces
    // the still-pending note.
    await byTestId(page, `background-run-steer-toggle-${runId}`).click()
    await expect(byTestId(page, `background-run-steer-${runId}`)).toHaveCount(0, {
      timeout: 10_000,
    })
    await byTestId(page, `background-run-steer-toggle-${runId}`).click()
    await expect(byTestId(page, `background-run-steer-${runId}`)).toBeVisible({ timeout: 10_000 })
    await expect(
      page.locator(`[data-testid^="background-run-note-"]`).filter({ hasText: noteText }).first(),
    ).toBeVisible({ timeout: 15_000 })
  })
})
