import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import {
  goToWorkflowsSettingsPage,
  openWorkflowCard,
  seedDevWorkflow,
} from './helpers/workflow-helpers'

/**
 * A workflow with a `kind: sandbox` step renders in the UI step list with its
 * sandbox kind tag + dependency. The sandbox step's RUNTIME dispatch + live
 * progress are covered by tests/workflow/sandbox_run.rs + sandbox_progress.rs
 * (the E2E harness disables code_sandbox, so a real browser-level sandbox run
 * isn't possible there); this covers the UI manifest rendering of a sandbox
 * step, which had no E2E.
 */
const SANDBOX_WF_YAML = `$schema: "/schemas/2026-06-12/workflow-definition.schema.json"
sandbox:
  flavor: minimal
inputs:
  - name: topic
    required: false
steps:
  - id: plan
    kind: llm
    prompt: "plan {{ inputs.topic }}"
  - id: process
    kind: sandbox
    stdin: "{{ plan.output }}"
    command: ["bash", "-lc", "tr a-z A-Z"]
    depends_on: [plan]
outputs:
  - name: out
    from: "{{ process.output }}"
    expose: full
`

test.describe('Workflows - sandbox step rendering', () => {
  test('the detail drawer lists a sandbox step with its kind tag + dependency', async ({
    page,
    request,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)

    const slug = `e2e-sandbox-step-${Date.now()}`
    await seedDevWorkflow(request, apiURL, adminToken, slug, SANDBOX_WF_YAML)

    await goToWorkflowsSettingsPage(page, baseURL)
    await openWorkflowCard(page, slug)

    // The drawer's step list renders both steps; the second carries the
    // `sandbox` kind tag and declares its dependency on the llm step.
    const drawer = page.getByRole('dialog')
    await expect(drawer.getByText('plan')).toBeVisible({ timeout: 15000 })
    await expect(drawer.getByText('process')).toBeVisible()
    await expect(drawer.getByText('sandbox', { exact: true })).toBeVisible()
    await expect(drawer.getByText(/depends on:\s*plan/)).toBeVisible()
  })
})
