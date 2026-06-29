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
  })
})
