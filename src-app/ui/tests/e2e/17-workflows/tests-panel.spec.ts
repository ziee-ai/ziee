import type { Page } from '@playwright/test'
import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import {
  goToWorkflowsSettingsPage,
  openWorkflowCard,
  seedDevWorkflow,
} from './helpers/workflow-helpers'

/**
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
    page,
    request,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
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
  })
})
