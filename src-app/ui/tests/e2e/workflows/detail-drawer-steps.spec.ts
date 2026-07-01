import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import {
  goToWorkflowsSettingsPage,
  openWorkflowCard,
  seedDevWorkflow,
} from './helpers/workflow-helpers'
import { byTestId } from '../testid'

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
    description: "Gather sources"
    prompt: |
      Find sources on "{{ inputs.topic }}".
  - id: synthesize
    kind: llm
    description: "Synthesize answer"
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

    // Both step messages render in the Steps list (dynamic data from the
    // seeded workflow), and the dependent step shows its dependency line.
    const dialog = byTestId(page, 'wf-detail-dialog')
    await expect(dialog).toContainText('Gather sources', { timeout: 15000 })
    await expect(dialog).toContainText('Synthesize answer')
    await expect(dialog).toContainText('depends on: gather', { timeout: 10000 })
  })
})
