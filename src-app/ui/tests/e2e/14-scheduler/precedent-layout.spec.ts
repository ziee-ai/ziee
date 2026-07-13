import { loginAsAdmin } from '../../common/auth-helpers'
import { expect, test } from '../../fixtures/test-context'
import { byTestId } from '../testid'
import { MODEL_ID, mockPickerEndpoints } from './helpers'

/**
 * E2E — Scheduled Tasks page precedent-fidelity (FB-9). Proves the top-level page
 * now renders inside the AppLayout shell (left sidebar + header bar) like
 * chat/projects/knowledge-base, paginates with Load-More, and has an actionable
 * empty state + hover-revealed card actions. These are B7 render proofs — each
 * asserts real DOM at /scheduled-tasks, not just a unit.
 */

function taskRow(overrides: Record<string, unknown> = {}) {
  return {
    id: '22222222-2222-2222-2222-222222222222',
    user_id: '00000000-0000-0000-0000-000000000001',
    name: 'A task',
    enabled: true,
    paused_reason: null,
    target_kind: 'prompt',
    workflow_id: null,
    inputs_json: {},
    assistant_id: null,
    prompt: 'Say hello.',
    model_id: MODEL_ID,
    schedule_kind: 'recurring',
    run_at: null,
    cron_expr: '0 9 * * 1',
    timezone: 'UTC',
    next_run_at: '2026-07-13T09:00:00Z',
    last_run_at: null,
    last_status: null,
    consecutive_failures: 0,
    notify_mode: 'always',
    notify_on: 'always',
    last_result_fingerprint: null,
    last_result_signature_json: null,
    bound_conversation_id: null,
    allowed_unattended_tools: [],
    created_at: '2026-07-09T00:00:00Z',
    updated_at: '2026-07-09T00:00:00Z',
    ...overrides,
  }
}

async function mockList(page: import('@playwright/test').Page, tasks: unknown[]) {
  await page.route(/\/api\/scheduled-tasks$/, route =>
    route.fulfill({ status: 200, json: tasks }),
  )
}

test('TEST-57: top-level page renders inside the AppLayout shell (sidebar + header bar)', async ({
  page,
  testInfra,
}) => {
  const { baseURL } = testInfra
  await mockPickerEndpoints(page)
  await mockList(page, [taskRow({ name: 'Shell task' })])

  await loginAsAdmin(page, baseURL)
  await page.goto(`${baseURL}/scheduled-tasks`)

  // AppLayout shell present (was absent when the route had no `layout:` and the
  // page used SettingsPageContainer instead of HeaderBarContainer).
  await expect(byTestId(page, 'app-sidebar')).toBeVisible({ timeout: 30000 })
  await expect(byTestId(page, 'app-header-bar')).toBeVisible({ timeout: 30000 })

  // The page header carries the title + the New-task action (in the header bar).
  await expect(byTestId(page, 'scheduled-tasks-title')).toHaveText('Scheduled Tasks')
  await expect(byTestId(page, 'scheduled-tasks-new')).toBeVisible()
})

test('TEST-58: task list pages with Load-More + "Showing N of M"', async ({
  page,
  testInfra,
}) => {
  const { baseURL } = testInfra
  await mockPickerEndpoints(page)
  const many = Array.from({ length: 15 }, (_, i) =>
    taskRow({ id: `1111111${String(i).padStart(1, '0')}-0000-0000-0000-000000000000`, name: `Task ${i}` }),
  )
  await mockList(page, many)

  await loginAsAdmin(page, baseURL)
  await page.goto(`${baseURL}/scheduled-tasks`)

  const cards = page.locator('[data-testid^="task-card-"]')
  await expect(cards).toHaveCount(12) // first page only
  await expect(byTestId(page, 'scheduled-tasks-paging')).toContainText('Showing 12 of 15')

  await byTestId(page, 'scheduled-tasks-load-more').click()
  await expect(cards).toHaveCount(15)
  await expect(byTestId(page, 'scheduled-tasks-paging')).toContainText('Showing 15 of 15')
  await expect(byTestId(page, 'scheduled-tasks-load-more')).toHaveCount(0)
})

test('TEST-59: empty state shows an icon + heading + create button that opens the drawer', async ({
  page,
  testInfra,
}) => {
  const { baseURL } = testInfra
  await mockPickerEndpoints(page)
  await mockList(page, [])

  await loginAsAdmin(page, baseURL)
  await page.goto(`${baseURL}/scheduled-tasks`)

  const empty = byTestId(page, 'scheduled-tasks-empty')
  await expect(empty).toBeVisible({ timeout: 10000 })
  await expect(empty).toContainText('No scheduled tasks yet')

  await byTestId(page, 'scheduled-tasks-empty-create').click()
  await expect(byTestId(page, 'task-form')).toBeVisible({ timeout: 10000 })
})

test('TEST-60: card exposes hover actions (edit opens the drawer) with the enable Switch always visible', async ({
  page,
  testInfra,
}) => {
  const { baseURL } = testInfra
  await mockPickerEndpoints(page)
  const row = taskRow({ name: 'Card task' })
  await mockList(page, [row])

  await loginAsAdmin(page, baseURL)
  await page.goto(`${baseURL}/scheduled-tasks`)

  const card = byTestId(page, `task-card-${row.id}`)
  await expect(card).toBeVisible({ timeout: 10000 })

  // Enable/disable Switch is STATE → always present.
  await expect(byTestId(page, `task-enabled-${row.id}`)).toBeVisible()

  // Actions are reachable (hover-revealed on desktop, always-on for touch): the
  // Edit action opens the drawer seeded with this task.
  await card.hover()
  await byTestId(page, `task-edit-${row.id}`).click()
  await expect(byTestId(page, 'task-form')).toBeVisible({ timeout: 10000 })
  await expect(byTestId(page, 'task-form-name')).toHaveValue('Card task')
})

test('TEST-61: the card title renders with the canonical list-item tokens (font-normal, text-sm), not bold', async ({
  page,
  testInfra,
}) => {
  const { baseURL } = testInfra
  await mockPickerEndpoints(page)
  const row = taskRow({ name: 'Typography task' })
  await mockList(page, [row])

  await loginAsAdmin(page, baseURL)
  await page.goto(`${baseURL}/scheduled-tasks`)

  const title = byTestId(page, `task-name-${row.id}`)
  await expect(title).toHaveText('Typography task')
  const { weight, size } = await title.evaluate(el => {
    const s = getComputedStyle(el)
    return { weight: Number(s.fontWeight), size: s.fontSize }
  })
  // Canonical list-item title: font-normal (400), text-sm (14px) — NOT medium/bold.
  expect(weight).toBeLessThanOrEqual(400)
  expect(size).toBe('14px')
})

test('TEST-63: at 390px there is no horizontal page scroll and the header/actions stay reachable', async ({
  page,
  testInfra,
}) => {
  const { baseURL } = testInfra
  await page.setViewportSize({ width: 390, height: 800 })
  await mockPickerEndpoints(page)
  const row = taskRow({ name: 'Mobile task' })
  await mockList(page, [row])

  await loginAsAdmin(page, baseURL)
  await page.goto(`${baseURL}/scheduled-tasks`)

  await expect(byTestId(page, 'scheduled-tasks-title')).toBeVisible({ timeout: 10000 })

  // No horizontal page overflow.
  const overflow = await page.evaluate(() => {
    const el = document.scrollingElement || document.documentElement
    return el.scrollWidth - el.clientWidth
  })
  expect(overflow).toBeLessThanOrEqual(1)

  // The card's actions remain reachable at mobile width.
  const card = byTestId(page, `task-card-${row.id}`)
  await card.hover()
  await expect(byTestId(page, `task-edit-${row.id}`)).toBeVisible()
})
