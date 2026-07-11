import { expect, test } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { byTestId } from '../testid'

/**
 * E2E — Paused-state surfacing + run history (ITEM-33): a task auto-paused after
 * repeated failures shows a "Paused" badge with its reason, and its "Runs"
 * section lists past firings with statuses. Mocks the list + runs endpoints.
 */

const TASK_ID = '44444444-4444-4444-4444-444444444444'

const pausedTask = {
  id: TASK_ID,
  user_id: '00000000-0000-0000-0000-000000000001',
  name: 'Flaky sweep',
  enabled: false,
  paused_reason: 'max_failures',
  target_kind: 'prompt',
  workflow_id: null,
  inputs_json: {},
  assistant_id: null,
  prompt: 'Sweep.',
  model_id: '11111111-1111-1111-1111-111111111111',
  schedule_kind: 'recurring',
  run_at: null,
  cron_expr: '0 9 * * 1',
  timezone: 'UTC',
  next_run_at: null,
  last_run_at: '2026-07-09T09:00:00Z',
  last_status: 'failed',
  consecutive_failures: 5,
  notify_mode: 'always',
  notify_on: 'always',
  last_result_fingerprint: null,
  last_result_signature_json: null,
  bound_conversation_id: null,
  created_at: '2026-07-01T00:00:00Z',
  updated_at: '2026-07-09T09:00:00Z',
}

test('paused task shows its reason and lists run history', async ({ page, testInfra }) => {
  const { baseURL } = testInfra

  await page.route(/\/api\/scheduled-tasks$/, async route =>
    route.fulfill({ status: 200, json: [pausedTask] }),
  )
  await page.route(/\/api\/scheduled-tasks\/[^/]+\/runs(\?.*)?$/, async route =>
    route.fulfill({
      status: 200,
      json: {
        runs: [
          {
            id: 'a1',
            scheduled_task_id: TASK_ID,
            user_id: pausedTask.user_id,
            fired_at: '2026-07-09T09:00:00Z',
            status: 'failed',
            error_class: 'provider_error',
            error_message: 'the provider returned 500',
            trigger: 'schedule',
            notification_id: null,
            workflow_run_id: null,
            conversation_id: null,
            result_preview: null,
            change_summary_json: null,
            skipped_tools: [],
          },
        ],
        total: 1,
        page: 1,
        per_page: 10,
      },
    }),
  )

  await loginAsAdmin(page, baseURL)
  await page.goto(`${baseURL}/scheduled-tasks`)

  // The paused badge with its reason renders.
  await expect(byTestId(page, `task-paused-${TASK_ID}`)).toContainText('max_failures', {
    timeout: 10000,
  })

  // Expand the runs section → the past firing shows a "Failed" badge; expanding the
  // run reveals its error message.
  await byTestId(page, `task-runs-toggle-${TASK_ID}`).click()
  await expect(byTestId(page, `run-badge-a1`)).toContainText('Failed', { timeout: 10000 })
  await byTestId(page, `run-expand-a1`).click()
  await expect(byTestId(page, `run-detail-a1`)).toContainText('provider_error', {
    timeout: 10000,
  })
})

// A spent `once` task: enabled=false with paused_reason='completed' (set by the
// tick when a once-task fires). This is DONE, not failed.
const completedTask = {
  ...pausedTask,
  id: '99999999-9999-9999-9999-999999999999',
  name: 'Once-off report',
  paused_reason: 'completed',
  schedule_kind: 'once',
  run_at: '2026-07-09T09:00:00Z',
  cron_expr: null,
  last_status: 'completed',
  consecutive_failures: 0,
}

test('TEST-4: a completed once-task shows a distinct "Completed" badge (not paused)', async ({
  page,
  testInfra,
}) => {
  const { baseURL } = testInfra
  await page.route(/\/api\/scheduled-tasks$/, async route =>
    route.fulfill({ status: 200, json: [completedTask] }),
  )

  await loginAsAdmin(page, baseURL)
  await page.goto(`${baseURL}/scheduled-tasks`)

  // A distinct "Completed" badge — never the error-toned "Paused" surface.
  await expect(byTestId(page, `task-completed-${completedTask.id}`)).toContainText(
    'Completed',
    { timeout: 10000 },
  )
  await expect(page.getByTestId(`task-paused-${completedTask.id}`)).toHaveCount(0)
})

test('TEST-32: a run with skipped_tools surfaces "N tools skipped"', async ({
  page,
  testInfra,
}) => {
  const { baseURL } = testInfra
  const runId = 'b2b2b2b2-0000-0000-0000-000000000001'

  await page.route(/\/api\/scheduled-tasks$/, async route =>
    route.fulfill({ status: 200, json: [pausedTask] }),
  )
  await page.route(/\/api\/scheduled-tasks\/[^/]+\/runs(\?.*)?$/, async route =>
    route.fulfill({
      status: 200,
      json: {
        runs: [
          {
            id: runId,
            scheduled_task_id: TASK_ID,
            user_id: pausedTask.user_id,
            trigger: 'schedule',
            status: 'completed',
            error_class: null,
            error_message: null,
            notification_id: null,
            workflow_run_id: null,
            conversation_id: null,
            result_preview: 'Swept OK.',
            change_summary_json: { changed: false, new_count: 0, new_items: [] },
            // Two tools were skipped because they weren't allow-listed unattended.
            skipped_tools: [
              { tool_name: 'write', reason: 'not permitted unattended' },
              { tool_name: 'delete', reason: 'not permitted unattended' },
            ],
            fired_at: '2026-07-09T09:00:00Z',
            finished_at: '2026-07-09T09:00:05Z',
          },
        ],
        total: 1,
        page: 1,
        per_page: 10,
      },
    }),
  )

  await loginAsAdmin(page, baseURL)
  await page.goto(`${baseURL}/scheduled-tasks`)

  // The skipped-tools note lives in the run's expanded detail.
  await byTestId(page, `task-runs-toggle-${TASK_ID}`).click()
  await byTestId(page, `run-expand-${runId}`).click()
  await expect(byTestId(page, `run-skipped-${runId}`)).toContainText(
    '2 tools skipped',
    { timeout: 10000 },
  )
})

// ── Round 2: follow-up & series timeline ────────────────────────────────────

const promptTask = {
  ...pausedTask,
  id: 'aaaaaaaa-0000-0000-0000-0000000000aa',
  name: 'Lit watch',
  enabled: true,
  paused_reason: null,
  bound_conversation_id: 'cccccccc-0000-0000-0000-0000000000cc',
}

function mkRun(i: number) {
  return {
    id: `run-${String(i).padStart(3, '0')}`,
    scheduled_task_id: promptTask.id,
    user_id: promptTask.user_id,
    trigger: 'schedule',
    status: 'completed',
    error_class: null,
    error_message: null,
    notification_id: null,
    workflow_run_id: null,
    conversation_id: null,
    result_preview: `Run ${i} found ${i % 3} new papers on base editing`,
    change_summary_json: { changed: i % 3 > 0, new_count: i % 3, new_items: [] },
    skipped_tools: [],
    fired_at: `2026-07-${String((i % 27) + 1).padStart(2, '0')}T09:00:00Z`,
    finished_at: null,
  }
}

// TEST-48 (ITEM-44): a run shows its what-changed badge + preview, and expands.
test('TEST-48: run row shows change badge + preview and expands', async ({ page, testInfra }) => {
  const { baseURL } = testInfra
  const run = mkRun(2) // new_count = 2 → "NEW ×2"
  await page.route(/\/api\/scheduled-tasks$/, r => r.fulfill({ status: 200, json: [promptTask] }))
  await page.route(/\/api\/scheduled-tasks\/[^/]+\/runs(\?.*)?$/, r =>
    r.fulfill({ status: 200, json: { runs: [run], total: 1, page: 1, per_page: 10 } }),
  )

  await loginAsAdmin(page, baseURL)
  await page.goto(`${baseURL}/scheduled-tasks`)
  await byTestId(page, `task-runs-toggle-${promptTask.id}`).click()

  await expect(byTestId(page, `run-badge-${run.id}`)).toContainText('NEW ×2', { timeout: 10000 })
  await expect(byTestId(page, `run-preview-${run.id}`)).toContainText('base editing')
  // Expand reveals the detail region.
  await byTestId(page, `run-expand-${run.id}`).click()
  await expect(byTestId(page, `run-detail-${run.id}`)).toBeVisible()
})

// TEST-52 (ITEM-46): >10 runs → 10 rows + "Showing 10 of N" + pagination; page 2.
test('TEST-52: runs panel paginates', async ({ page, testInfra }) => {
  const { baseURL } = testInfra
  const all = Array.from({ length: 25 }, (_, i) => mkRun(i + 1))
  await page.route(/\/api\/scheduled-tasks$/, r => r.fulfill({ status: 200, json: [promptTask] }))
  await page.route(/\/api\/scheduled-tasks\/[^/]+\/runs(\?.*)?$/, async route => {
    const url = new URL(route.request().url())
    const pg = Number(url.searchParams.get('page') ?? '1')
    const per = Number(url.searchParams.get('per_page') ?? '10')
    const slice = all.slice((pg - 1) * per, pg * per)
    await route.fulfill({
      status: 200,
      json: { runs: slice, total: all.length, page: pg, per_page: per },
    })
  })

  await loginAsAdmin(page, baseURL)
  await page.goto(`${baseURL}/scheduled-tasks`)
  await byTestId(page, `task-runs-toggle-${promptTask.id}`).click()

  await expect(byTestId(page, `runs-count-${promptTask.id}`)).toContainText('Showing 10 of 25', {
    timeout: 10000,
  })
  await expect(byTestId(page, `runs-pagination-${promptTask.id}`)).toBeVisible()
  // First page shows run-001; go to page 2 → run-011 appears.
  await expect(byTestId(page, 'run-row-run-001')).toBeVisible()
  await page.getByLabel('Page 2').first().click()
  await expect(byTestId(page, 'run-row-run-011')).toBeVisible({ timeout: 10000 })
})

// TEST-54 (ITEM-47): "Discuss recent runs" chooser calls continue-series.
test('TEST-54: discuss recent runs calls continue-series', async ({ page, testInfra }) => {
  const { baseURL } = testInfra
  let seriesLimit: string | null = null
  await page.route(/\/api\/scheduled-tasks$/, r => r.fulfill({ status: 200, json: [promptTask] }))
  await page.route(/\/api\/scheduled-tasks\/[^/]+\/runs(\?.*)?$/, r =>
    r.fulfill({ status: 200, json: { runs: [mkRun(1), mkRun(2)], total: 2, page: 1, per_page: 10 } }),
  )
  await page.route(/\/api\/scheduled-tasks\/[^/]+\/continue-series(\?.*)?$/, async route => {
    seriesLimit = new URL(route.request().url()).searchParams.get('limit')
    await route.fulfill({ status: 201, json: { conversation_id: 'dddddddd-0000-0000-0000-0000000000dd' } })
  })

  await loginAsAdmin(page, baseURL)
  await page.goto(`${baseURL}/scheduled-tasks`)
  await byTestId(page, `task-runs-toggle-${promptTask.id}`).click()

  // Open the "Discuss recent runs" menu and pick "Last 5".
  await byTestId(page, `series-chooser-${promptTask.id}`).click()
  await page.getByRole('menuitem', { name: 'Last 5' }).click()
  await expect.poll(() => seriesLimit, { timeout: 10000 }).toBe('5')
})

// TEST-56 (ITEM-48): at a 390px viewport the per-run actions collapse to an
// overflow menu and the page does not scroll horizontally.
test('TEST-56: responsive — mobile overflow menu, no horizontal scroll', async ({ page, testInfra }) => {
  const { baseURL } = testInfra
  const run = mkRun(1)
  await page.setViewportSize({ width: 390, height: 844 })
  await page.route(/\/api\/scheduled-tasks$/, r => r.fulfill({ status: 200, json: [promptTask] }))
  await page.route(/\/api\/scheduled-tasks\/[^/]+\/runs(\?.*)?$/, r =>
    r.fulfill({ status: 200, json: { runs: [run], total: 1, page: 1, per_page: 10 } }),
  )

  await loginAsAdmin(page, baseURL)
  await page.goto(`${baseURL}/scheduled-tasks`)
  await byTestId(page, `task-runs-toggle-${promptTask.id}`).click()

  // The overflow menu trigger is present; the inline desktop buttons are hidden.
  await expect(byTestId(page, `run-actions-menu-${run.id}`)).toBeVisible({ timeout: 10000 })
  await expect(byTestId(page, `run-action-fork-${run.id}`)).toBeHidden()
  // No horizontal page scroll at mobile width.
  const overflow = await page.evaluate(
    () => document.documentElement.scrollWidth <= document.documentElement.clientWidth + 1,
  )
  expect(overflow).toBe(true)
})
