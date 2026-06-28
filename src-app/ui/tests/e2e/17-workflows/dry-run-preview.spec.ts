import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import {
  goToWorkflowsSettingsPage,
  openWorkflowCard,
  seedDevWorkflow,
} from './helpers/workflow-helpers'

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
  await page.getByRole('button', { name: 'Dry-run preview' }).click()
}

test.describe('Workflows — dry-run preview dialog', () => {
  test.describe.configure({ retries: 2 })

  test('shows the loading spinner then the estimate on success', async ({
 * Dry-run preview — `DryRunPreviewDialog` E2E (audit gap all-892d43854b43).
 *
 * The dialog (DryRunPreviewDialog.tsx) had zero E2E coverage: opening it
 * fires a REAL `POST /api/workflows/{id}/dry-run`, then renders the
 * total est-calls/est-tokens Statistics plus a per-step Table with a
 * "runtime-dependent" marker on fan-out steps whose count can't be known
 * statically. Dry-run spends ZERO tokens and needs NO provider/model — it's
 * a static walk of the steps (cost.rs::dry_run), so this test is fully
 * deterministic (no real LLM, no mocks).
 *
 * The seeded workflow is the exact shape `cost.rs::dry_run_marks_prior_step
 * _for_each_runtime_dependent` exercises on the backend: a `gen` llm step
 * (1 est call) + a `fan` llm_map step whose `for_each` references the prior
 * step's output, so the count is unknowable at dry-run time → it falls back
 * to `max_parallel` (6) and is flagged runtime-dependent. Total = 1 + 6 = 7.
 */

const DRY_RUN_WORKFLOW_YAML = `$schema: "/schemas/2026-06-12/workflow-definition.schema.json"
steps:
  - id: gen
    kind: llm
    message: "Generate a list"
    prompt: "List three things."
  - id: fan
    kind: llm_map
    message: "Fan out over the list"
    for_each: "{{ gen.output }}"
    item_var: q
    prompt: "{{ q }}"
    max_parallel: 6
    depends_on: [gen]
`

test.describe('Workflows — dry-run preview', () => {
  test('opening the dialog calls /dry-run and renders per-step estimates', async ({
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

    const dialog = page.getByRole('dialog').filter({ hasText: 'Dry-run preview' })
    // Loading state: the Spin is visible while the request is in flight.
    await expect(dialog.locator('.ant-spin')).toBeVisible({ timeout: 5000 })
    // Then the estimate renders.
    await expect(dialog.getByText('Est. calls')).toBeVisible({ timeout: 10000 })
    await expect(dialog.getByText('only_step')).toBeVisible()
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

    const dialog = page.getByRole('dialog').filter({ hasText: 'Dry-run preview' })
    await expect(dialog.locator('.ant-alert-error')).toBeVisible({ timeout: 10000 })
    const adminToken = await getAdminToken(apiURL)

    const slug = `dryrun-preview-${Date.now().toString(36)}`
    await seedDevWorkflow(
      request,
      apiURL,
      adminToken,
      slug,
      DRY_RUN_WORKFLOW_YAML,
    )

    await goToWorkflowsSettingsPage(page, baseURL)
    await openWorkflowCard(page, slug)

    // Open the dialog and capture the REAL dry-run request/response.
    const dryRunResp = page.waitForResponse(
      r =>
        /\/api\/workflows\/[^/]+\/dry-run$/.test(r.url()) &&
        r.request().method() === 'POST',
    )
    await page.getByRole('button', { name: /Dry-run preview/i }).click()
    const resp = await dryRunResp
    expect(resp.status(), 'dry-run should 200').toBe(200)

    // The dialog renders the modal title + the two always-present Statistics.
    const dialog = page.getByRole('dialog', { name: /Dry-run preview/i })
    await expect(dialog).toBeVisible({ timeout: 15000 })
    await expect(dialog.getByText('Est. calls', { exact: true })).toBeVisible()
    await expect(dialog.getByText('Est. tokens', { exact: true })).toBeVisible()

    // Total est calls = gen(1) + fan(max_parallel 6) = 7. antd Statistic
    // renders the integer (no grouping for a value this small).
    await expect(
      dialog.locator('.ant-statistic-content-value', { hasText: /^7$/ }),
    ).toBeVisible()

    // Per-step table: both step ids surface as their own rows…
    await expect(dialog.getByRole('cell', { name: 'gen', exact: true })).toBeVisible()
    await expect(dialog.getByRole('cell', { name: 'fan', exact: true })).toBeVisible()
    // …and the fan-out step is flagged runtime-dependent (its count can't be
    // resolved statically because for_each references a prior step's output).
    await expect(dialog.getByText('runtime-dependent').first()).toBeVisible()
  })
})
