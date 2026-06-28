import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import {
  goToWorkflowsSettingsPage,
  openWorkflowCard,
  seedDevWorkflow,
} from './helpers/workflow-helpers'

/**
 * E2E — WorkflowDetailDrawer step list + dependency rendering
 * (WorkflowDetailDrawer.tsx:177-202). The drawer parses the workflow IR and
 * renders a vertical Steps list; a step with `depends_on` shows a
 * "depends on: <ids>" line. Untested. No run needed — pure IR display.
 */

const TWO_STEP_YAML = `$schema: "/schemas/2026-06-12/workflow-definition.schema.json"
inputs:
  - name: topic
    required: true
steps:
  - id: gather
    kind: llm
    message: "Gather sources"
    prompt: |
      Find sources on "{{ inputs.topic }}".
  - id: synthesize
    kind: llm
    message: "Synthesize answer"
    prompt: |
      Synthesize from the gathered sources.
    depends_on: [gather]
outputs:
  - name: out
    from: "{{ synthesize.output }}"
    expose: full
`

test.describe('Workflows - detail drawer steps', () => {
  test('the detail drawer lists steps and shows the dependency line', async ({
    page,
    request,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    await seedDevWorkflow(request, apiURL, token, 'e2e-detail-steps', TWO_STEP_YAML)

    await goToWorkflowsSettingsPage(page, baseURL)
    await openWorkflowCard(page, 'e2e-detail-steps')

    // Both step messages render in the Steps list.
    await expect(page.getByText('Gather sources')).toBeVisible({ timeout: 15000 })
    await expect(page.getByText('Synthesize answer')).toBeVisible()

    // The dependent step shows its dependency line.
    await expect(page.getByText('depends on: gather')).toBeVisible({ timeout: 10000 })
  })
})
