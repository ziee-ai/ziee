import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  createModelViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'
import {
  goToWorkflowsSettingsPage,
  openWorkflowCard,
  seedDevWorkflow,
} from './helpers/workflow-helpers'

/**
 * D2/D3 — the rich editable-array-table elicit widget.
 *
 * Drives the full UI flow:
 *   seed a 2-step workflow (`screen` llm → `elicit` with `data:"{{screen.output}}"`
 *   + an array-of-object schema with `ui:` table hints) → start the run via the
 *   API with the `screen` step MOCKED (a deterministic 3-row array; no token
 *   spent) and a real `model_id` (only snapshotted) → the run pauses on the
 *   elicit step → open it in the Runs tab → the table renders the seeded rows →
 *   edit a cell, bulk-toggle include, expand a row → Submit → the run resumes.
 *
 * Real-LLM tier only because the run still snapshots a model at start; the
 * actual screening output is mocked, so the table content is deterministic.
 * Backend coverage of the seed + submit contracts:
 * `tests/workflow/elicit_data_seeding.rs`.
 */

const ANTHROPIC_KEY = process.env.ANTHROPIC_API_KEY ?? ''
const HAS_ANTHROPIC = ANTHROPIC_KEY.length > 0

// `screen` (llm, mocked) → `review` (elicit, rich table seeded from screen).
// The `ui:` hints opt the `include` column into a bulk-toggle and the
// `abstract` column into expand — the schema still validates without them.
const TABLE_WORKFLOW_YAML = `$schema: "/schemas/2026-06-12/workflow-definition.schema.json"
inputs:
  - name: topic
    required: false
steps:
  - id: screen
    kind: llm
    prompt: "screen {{ inputs.topic }}"
    output_format: json
  - id: review
    kind: elicit
    message: "Review the screened papers"
    data: "{{ screen.output }}"
    schema:
      type: object
      properties:
        rows:
          type: array
          ui: { widget: table }
          items:
            type: object
            properties:
              title:
                type: string
              abstract:
                type: string
                ui: { expand: true }
              include:
                type: boolean
                ui: { bulkToggle: true }
            required: [include]
      required: [rows]
    timeout_ms: 600000
    depends_on: [screen]
outputs:
  - name: decision
    from: "{{ review.output }}"
    expose: full
`

// The deterministic mocked screening output — an object with a `rows` array so
// it seeds the elicit form's `rows` property directly.
const SEEDED_ROWS = {
  rows: [
    { title: 'Paper A', abstract: 'A long abstract about A '.repeat(8), include: true },
    { title: 'Paper B', abstract: 'A long abstract about B '.repeat(8), include: false },
    { title: 'Paper C', abstract: 'A long abstract about C '.repeat(8), include: false },
  ],
}

test.describe('Workflows - elicit rich editable table (real LLM snapshot)', () => {
  test.skip(!HAS_ANTHROPIC, 'ANTHROPIC_API_KEY not set — real-LLM E2E skipped')

  test('seed → edit cell → bulk-toggle → expand → submit → resume', async ({
    page,
    request,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra

    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)

    const providerId = await createProviderViaAPI(
      apiURL,
      adminToken,
      'Anthropic',
      'anthropic',
    )
    await assignProviderToAdministratorsGroup(apiURL, adminToken, providerId)
    const modelId = await createModelViaAPI(
      apiURL,
      adminToken,
      providerId,
      'claude-haiku-4-5-20251001',
      'Claude Haiku 4.5',
      'anthropic',
    )

    const workflowId = await seedDevWorkflow(
      request,
      apiURL,
      adminToken,
      'e2e-elicit-table',
      TABLE_WORKFLOW_YAML,
    )

    // Start the run via the API: mock the `screen` step (deterministic rows) +
    // pass the real model_id (only snapshotted). The run pauses on `review`.
    const runResp = await request.post(
      `${apiURL}/api/workflows/${workflowId}/run`,
      {
        headers: { Authorization: `Bearer ${adminToken}` },
        data: {
          inputs: { topic: 'x' },
          model_id: modelId,
          mocks: { screen: SEEDED_ROWS },
        },
      },
    )
    expect(runResp.status(), `run should 202: ${await runResp.text()}`).toBe(202)

    // Open the workflow, then its (paused) run in the Runs tab.
    await goToWorkflowsSettingsPage(page, baseURL)
    await openWorkflowCard(page, 'e2e-elicit-table')

    // The running run shows in the Runs list; open it.
    await expect(page.getByText('Runs', { exact: true })).toBeVisible()
    await page.getByText('Workflow page', { exact: true }).first().click()

    // The elicit form renders ("Input required") with the seeded table rows.
    await expect(page.getByText(/input required/i)).toBeVisible({
      timeout: 15000,
    })
    // The seeded titles appear (the table pre-filled from `data:`).
    await expect(page.getByText('Paper A')).toBeVisible({ timeout: 10000 })
    await expect(page.getByText('Paper C')).toBeVisible()

    // Bulk-toggle: select all rows, then "Set include on" for the bulk column.
    // The header checkbox selects every row.
    const headerCheckbox = page.getByRole('checkbox').first()
    await headerCheckbox.check()
    await page
      .getByRole('button', { name: /set include on/i })
      .first()
      .click()

    // Expand a row to reveal the long `abstract` (expand affordance is the
    // first-column expand icon; the expanded content shows the abstract text).
    const expandIcon = page.locator('.ant-table-row-expand-icon').first()
    if (await expandIcon.count()) {
      await expandIcon.click()
    }

    // Edit a cell: flip the include switch off for the first row to prove cells
    // are editable (any in-table switch).
    const rowSwitch = page.getByRole('switch').first()
    if (await rowSwitch.count()) {
      await rowSwitch.click()
    }

    // Submit the form → the run resumes.
    await page.getByRole('button', { name: 'Submit', exact: true }).click()

    // After resume the single elicit-after step completes the run.
    await expect(page.getByText('completed', { exact: true })).toBeVisible({
      timeout: 30000,
    })
  })
})
