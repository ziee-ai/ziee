import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import {
  goToWorkflowsSettingsPage,
  openWorkflowCard,
  seedDevWorkflow,
} from './helpers/workflow-helpers'
import { byTestId } from '../testid'

/**
 * E2E — DryRunPreviewDialog loading + error states (audit id 80086b394b17).
 * The dialog calls POST /api/workflows/{id}/dry-run on open and renders a Spin
 * while pending, an error Alert on failure, and the estimate table on success.
 * Those branches were untested. We mock ONLY the dry-run endpoint to drive each
 * state deterministically; the dialog open/render path runs for real.
 */

const SLUG = 'dryrun-preview'
const YAML = `$schema: "/schemas/2026-06-12/workflow-definition.schema.json"
inputs: []
steps:
  - id: only_step
    kind: llm
    prompt: "say hi"
outputs: []
`

async function openDryRun(page: import('@playwright/test').Page) {
  await openWorkflowCard(page, SLUG)
  await byTestId(page, 'wf-detail-dry-run-btn').click()
}

test.describe('Workflows — dry-run preview dialog', () => {
  test.describe.configure({ retries: 2 })

  test('shows the loading spinner then the estimate on success', async ({
    page,
    request,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    await seedDevWorkflow(request, apiURL, token, SLUG, YAML)

    // Mock dry-run with a deliberate delay so the loading Spin is observable.
    await page.route(/\/api\/workflows\/[^/]+\/dry-run$/, async route => {
      await new Promise(r => setTimeout(r, 1500))
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({
          total_est_calls: 1,
          total_est_tokens: 1234,
          est_cost_usd: 0.0021,
          steps: [
            {
              step_id: 'only_step',
              kind: 'llm',
              est_calls: 1,
              est_tokens_in: 800,
              est_tokens_out: 434,
              runtime_dependent: false,
            },
          ],
        }),
      })
    })

    await goToWorkflowsSettingsPage(page, baseURL)
    await openDryRun(page)

    const dialog = byTestId(page, 'wf-dry-run-dialog')
    // Loading state: the Spin is visible while the request is in flight.
    await expect(byTestId(dialog, 'wf-dry-run-spin')).toBeVisible({
      timeout: 5000,
    })
    // Then the estimate renders.
    await expect(byTestId(dialog, 'wf-dry-run-stat-calls')).toBeVisible({
      timeout: 10000,
    })
    // The seeded step id appears in the estimate table (dynamic test data).
    await expect(byTestId(dialog, 'wf-dry-run-steps-table')).toContainText(
      'only_step',
    )
  })

  test('shows an error alert when dry-run fails', async ({
    page,
    request,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    await seedDevWorkflow(request, apiURL, token, SLUG, YAML)

    await page.route(/\/api\/workflows\/[^/]+\/dry-run$/, async route =>
      route.fulfill({
        status: 500,
        contentType: 'application/json',
        body: JSON.stringify({ error_code: 'INTERNAL', error: 'estimator exploded' }),
      }),
    )

    await goToWorkflowsSettingsPage(page, baseURL)
    await openDryRun(page)

    const dialog = byTestId(page, 'wf-dry-run-dialog')
    await expect(byTestId(dialog, 'wf-dry-run-error-alert')).toBeVisible({
      timeout: 10000,
    })
  })
})
