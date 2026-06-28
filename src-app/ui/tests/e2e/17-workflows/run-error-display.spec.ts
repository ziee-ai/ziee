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
 * Error-path coverage for the run UI (audit gap all-d9d8b6762eaa).
 *
 * `WorkflowRunProgressView` renders an antd `Alert type="error"` whose title
 * is `run.error` when a run ends in a failed state, and `WorkflowRunDialog`
 * surfaces a start-time error via `message.error`. Nothing exercised the
 * failed-run Alert branch.
 *
 * This drives a REAL failed run — no mock, no LLM, no API key. The seeded
 * workflow is a single `kind: tool` step that targets an MCP server which
 * doesn't exist for this user. At dispatch `resolve_tool_server` returns
 * `WORKFLOW_TOOL_SERVER_NOT_ACCESSIBLE` ("server '...' is not accessible to
 * this user"), so the step fails and the run reaches `failed` BEFORE any
 * model is touched — fully deterministic. A model is still registered so the
 * standalone Run dialog's model picker has an option (the model is never
 * called: there is no `llm` step).
 */

// A single tool step pointing at a non-existent server — fails at dispatch.
const FAILING_WORKFLOW_YAML = `$schema: "/schemas/2026-06-12/workflow-definition.schema.json"
inputs: []
steps:
  - id: call_missing
    kind: tool
    server: nonexistent_server_e2e
    tool: do_something
    arguments: {}
outputs:
  - name: result
    from: "{{ call_missing.output }}"
    expose: full
`

test.describe('Workflows - failed run surfaces an error Alert', () => {
  test('a tool step on a missing server drives the run to failed + error Alert', async ({
    page,
    request,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra

    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)

    // A model just so the standalone Run dialog's picker is non-empty. The
    // workflow has no llm step, so this model is never actually invoked —
    // a dummy provider key is fine and keeps the test off the real-LLM tier.
    const providerId = await createProviderViaAPI(
      apiURL,
      adminToken,
      'OpenAI',
      'openai',
    )
    await assignProviderToAdministratorsGroup(apiURL, adminToken, providerId)
    await createModelViaAPI(
      apiURL,
      adminToken,
      providerId,
      undefined,
      undefined,
      'openai',
    )

    await seedDevWorkflow(
      request,
      apiURL,
      adminToken,
      'e2e-failing-run',
      FAILING_WORKFLOW_YAML,
    )

    await goToWorkflowsSettingsPage(page, baseURL)
    await openWorkflowCard(page, 'e2e-failing-run')

    // Open the Run dialog from the drawer (drawer button name is the icon
    // aria-label + "Run"; match the trailing word).
    await page.getByRole('button', { name: /Run$/ }).first().click()
    await expect(page.getByRole('dialog', { name: /^Run / })).toBeVisible({
      timeout: 10000,
    })

    // Pick the registered model (required for a standalone run).
    const modelSelect = page.getByLabel('Model')
    await modelSelect.click()
    await page.getByRole('option').first().click()

    // Kick the run (the dialog's own OK "Run" button).
    await page.getByRole('button', { name: 'Run', exact: true }).last().click()

    // Run-progress view appears, the run transitions to `failed`, and the
    // error Alert renders the dispatch error text. The Alert title is the
    // raw `run.error`; assert on the stable substring from
    // resolve_tool_server's WORKFLOW_TOOL_SERVER_NOT_ACCESSIBLE message.
    await expect(page.getByText('Run progress')).toBeVisible({ timeout: 15000 })
    await expect(
      page.getByText('failed', { exact: true }).first(),
    ).toBeVisible({ timeout: 30000 })

    const errorAlert = page.locator('.ant-alert-error')
    await expect(errorAlert).toBeVisible({ timeout: 30000 })
    await expect(errorAlert).toContainText(/not accessible|nonexistent_server_e2e/i)
  })
})
