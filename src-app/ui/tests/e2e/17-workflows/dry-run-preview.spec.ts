import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import {
  goToWorkflowsSettingsPage,
  openWorkflowCard,
  seedDevWorkflow,
} from './helpers/workflow-helpers'

/**
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
