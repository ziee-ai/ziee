import type { Page } from '@playwright/test'
import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import {
  goToWorkflowsSettingsPage,
  openWorkflowCard,
  seedDevWorkflow,
} from './helpers/workflow-helpers'

/**
 * E2E — WorkflowTestsPanel (audit id c3b0bc0c903ade49). The panel calls
 * POST /api/workflows/{id}/test on open and renders a Spin while pending, an
 * error Alert on failure, and per-fixture pass/fail (with failure detail) on
 * success. Those branches had no E2E coverage. We mock ONLY the /test endpoint;
 * the panel open/render path runs for real. "Run tests" only shows on dev
 * (imported) workflows.
 */

const SLUG = 'tests-panel'
const YAML = `$schema: "/schemas/2026-06-12/workflow-definition.schema.json"
inputs: []
steps:
  - id: only_step
    kind: llm
    prompt: "say hi"
outputs: []
`

async function openTests(page: import('@playwright/test').Page) {
  await openWorkflowCard(page, SLUG)
  await page.getByRole('button', { name: 'Run tests' }).click()
}

test.describe('Workflows — tests panel', () => {
  test.describe.configure({ retries: 2 })

  test('renders per-fixture pass/fail (incl. a failure detail) on success', async ({
 * E2E — WorkflowTestsPanel renders pass/fail/skip counts + per-fixture list.
 *
 * The panel loads on mount (POST /api/workflows/{id}/test), shows a spinner,
 * then renders green/red/grey Tag counts and a List with one item per fixture.
 * The test-RUN result is route-mocked (the external boundary — the runner is
 * unit-tested in `workflow/test_runner.rs`); the behavior under test is the
 * panel's own rendering of that result.
 */

const TINY_DEV_YAML = `$schema: "/schemas/2026-06-12/workflow-definition.schema.json"
inputs:
  - name: topic
    required: true
steps:
  - id: echo
    kind: llm
    message: "echo {{ inputs.topic }}"
    prompt: |
      Say "{{ inputs.topic }}".
outputs:
  - name: out
    from: "{{ echo.output }}"
    expose: full
`

async function mockTestRun(page: Page) {
  await page.route(/\/api\/workflows\/[^/]+\/test$/, async (route, req) => {
    if (req.method() !== 'POST') return route.continue()
    return route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({
        total: 3,
        passed: 1,
        failed: 1,
        skipped: 1,
        results: [
          { name: 'happy-path', passed: true, skipped: false, duration_ms: 12 },
          {
            name: 'bad-output',
            passed: false,
            skipped: false,
            duration_ms: 8,
            failure: {
              output_name: 'out',
              assertion: 'equals',
              expected: '"hello"',
              actual_preview: '"world"',
            },
          },
          {
            name: 'real-llm-case',
            passed: false,
            skipped: true,
            duration_ms: 0,
          },
        ],
      }),
    })
  })
}

test.describe('Workflows - tests panel', () => {
  test('renders pass/fail/skip tag counts and a per-fixture list', async ({
 * WorkflowTestsPanel: opened from the detail drawer's "Run tests" button, it
 * loads test results on mount (POST /workflows/{id}/test → spinner) and renders
 * pass/fail/skip Tag counts + per-fixture rows. The test EXECUTION is the
 * external boundary, mocked here; the PANEL rendering is the behavior under test.
 */
const WF_YAML = `$schema: "/schemas/2026-06-12/workflow-definition.schema.json"
inputs:
  - name: topic
    required: false
steps:
  - id: noop
    kind: llm
    prompt: "about {{ inputs.topic }}"
outputs:
  - name: out
    from: "{{ noop.output }}"
    expose: full
`

test.describe('Workflows - tests panel', () => {
  test('Run tests opens the panel and renders pass/fail/skip counts', async ({
    page,
    request,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    await seedDevWorkflow(request, apiURL, token, SLUG, YAML)

    await page.route(/\/api\/workflows\/[^/]+\/test$/, async route => {
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({
          total: 2,
          passed: 1,
          failed: 1,
          skipped: 0,
          results: [
            { name: 'fixture_ok', passed: true, duration_ms: 12 },
            {
              name: 'fixture_bad',
              passed: false,
              duration_ms: 9,
              failure: {
                output_name: 'summary',
                assertion: 'contains',
                expected: 'hello',
                actual_preview: 'goodbye',
              },
            },
          ],
        }),
      })
    })

    await goToWorkflowsSettingsPage(page, baseURL)
    await openTests(page)

    const dialog = page.getByRole('dialog').filter({ hasText: 'Workflow tests' })
    // Summary tags.
    await expect(dialog.getByText('1 passed')).toBeVisible({ timeout: 10000 })
    await expect(dialog.getByText('1 failed')).toBeVisible()
    // Per-fixture rows, including the failure detail.
    await expect(dialog.getByText('fixture_ok')).toBeVisible()
    await expect(dialog.getByText('fixture_bad')).toBeVisible()
    await expect(dialog.getByText(/summary: contains/)).toBeVisible()
  })

  test('shows an error alert when the test run fails', async ({
    page,
    request,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    await seedDevWorkflow(request, apiURL, token, SLUG, YAML)

    await page.route(/\/api\/workflows\/[^/]+\/test$/, async route =>
      route.fulfill({
        status: 500,
        contentType: 'application/json',
        body: JSON.stringify({ error_code: 'INTERNAL', error: 'test runner exploded' }),
      }),
    )

    await goToWorkflowsSettingsPage(page, baseURL)
    await openTests(page)

    const dialog = page.getByRole('dialog').filter({ hasText: 'Workflow tests' })
    await expect(dialog.locator('.ant-alert-error')).toBeVisible({ timeout: 10000 })
    const adminToken = await getAdminToken(apiURL)

    await seedDevWorkflow(request, apiURL, adminToken, 'e2e-tests-panel', TINY_DEV_YAML)
    await mockTestRun(page)

    await goToWorkflowsSettingsPage(page, baseURL)
    await openWorkflowCard(page, 'e2e-tests-panel')

    // Open the dev "Run tests" panel.
    await page.getByRole('button', { name: /Run tests/ }).click()
    const modal = page.getByRole('dialog', { name: 'Workflow tests' })
    await expect(modal).toBeVisible({ timeout: 10000 })

    // Tag counts from the mocked result.
    await expect(modal.getByText('1 passed')).toBeVisible({ timeout: 10000 })
    await expect(modal.getByText('1 failed')).toBeVisible()
    await expect(modal.getByText('1 skipped')).toBeVisible()

    // Per-fixture list items.
    await expect(modal.getByText('happy-path')).toBeVisible()
    await expect(modal.getByText('bad-output')).toBeVisible()
    await expect(modal.getByText('real-llm-case')).toBeVisible()
    // The failing fixture surfaces its assertion detail.
    await expect(modal.getByText(/expected "hello"; got "world"/)).toBeVisible()
    const adminToken = await getAdminToken(apiURL)
    const slug = `e2e-tests-panel-${Date.now()}`
    await seedDevWorkflow(request, apiURL, adminToken, slug, WF_YAML)

    // Mock the test-run result (the boundary that executes fixtures).
    await page.route(/\/api\/workflows\/[0-9a-f-]+\/test$/, async (route, req) => {
      if (req.method() === 'POST') {
        return route.fulfill({
          status: 200,
          contentType: 'application/json',
          body: JSON.stringify({
            total: 4,
            passed: 2,
            failed: 1,
            skipped: 1,
            results: [
              { name: 'fixture-ok-1', passed: true, duration_ms: 12 },
              { name: 'fixture-ok-2', passed: true, duration_ms: 9 },
              {
                name: 'fixture-bad',
                passed: false,
                duration_ms: 7,
                failure: { message: 'assertion failed' } as any,
              },
              { name: 'fixture-skipped', passed: false, skipped: true, duration_ms: 0 },
            ],
          }),
        })
      }
      return route.continue()
    })

    await goToWorkflowsSettingsPage(page, baseURL)
    await openWorkflowCard(page, slug)

    await page.getByRole('button', { name: 'Run tests' }).click()

    // The panel renders the aggregate pass/fail/skip Tag counts...
    await expect(page.getByText('2 passed')).toBeVisible({ timeout: 10000 })
    await expect(page.getByText('1 failed')).toBeVisible()
    await expect(page.getByText('1 skipped')).toBeVisible()
    // ...and the per-fixture rows.
    await expect(page.getByText('fixture-ok-1')).toBeVisible()
    await expect(page.getByText('fixture-bad')).toBeVisible()
  })
})
