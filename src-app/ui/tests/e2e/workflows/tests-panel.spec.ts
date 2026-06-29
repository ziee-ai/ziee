import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import {
  goToWorkflowsSettingsPage,
  openWorkflowCard,
  seedDevWorkflow,
} from './helpers/workflow-helpers'
import { byTestId } from '../testid'

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
  await byTestId(page, 'wf-detail-run-tests-btn').click()
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

    const dialog = byTestId(page, 'wf-tests-dialog')
    // Summary tags (counts are dynamic data from the mocked response).
    await expect(byTestId(dialog, 'wf-tests-passed-tag')).toContainText(
      '1 passed',
      { timeout: 10000 },
    )
    await expect(byTestId(dialog, 'wf-tests-failed-tag')).toContainText(
      '1 failed',
    )
    // Per-fixture rows, including the failure detail (dynamic test data).
    const list = byTestId(dialog, 'wf-tests-list')
    await expect(list).toContainText('fixture_ok')
    await expect(list).toContainText('fixture_bad')
    await expect(list).toContainText(/summary: contains/)
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

    const dialog = byTestId(page, 'wf-tests-dialog')
    await expect(byTestId(dialog, 'wf-tests-error-alert')).toBeVisible({
      timeout: 10000,
    })
  })
})
