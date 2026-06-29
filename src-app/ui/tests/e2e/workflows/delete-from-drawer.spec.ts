import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import {
  goToWorkflowsSettingsPage,
  openWorkflowCard,
  seedDevWorkflow,
} from './helpers/workflow-helpers'
import { byTestId } from '../testid'

/**
 * Workflow deletion from the detail drawer (WorkflowDetailDrawer handleDelete +
 * Popconfirm). run-history.spec covers deleting a RUN; this covers deleting the
 * WORKFLOW itself. No model/run needed — pure CRUD.
 */
const WF_YAML = `$schema: "/schemas/2026-06-12/workflow-definition.schema.json"
inputs:
  - name: topic
    required: false
steps:
  - id: noop
    kind: llm
    prompt: "say something about {{ inputs.topic }}"
outputs:
  - name: out
    from: "{{ noop.output }}"
    expose: full
`

test.describe('Workflows - delete from detail drawer', () => {
  test('deleting a workflow via the drawer Popconfirm removes it from the list', async ({
    page,
    request,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)

    const slug = `e2e-del-wf-${Date.now()}`
    await seedDevWorkflow(request, apiURL, adminToken, slug, WF_YAML)

    await goToWorkflowsSettingsPage(page, baseURL)
    await openWorkflowCard(page, slug)

    // The drawer's Delete button → confirm dialog "Delete this workflow?".
    await byTestId(page, 'wf-detail-delete-btn').click()
    await expect(byTestId(page, 'wf-detail-delete-dialog')).toBeVisible()
    // Confirm via the danger OK button (okText "Delete").
    await byTestId(page, 'wf-detail-delete-confirm-btn').click()

    // Success toast + the drawer closes + the card is gone from the list.
    await expect(
      page.locator('[data-sonner-toast][data-type="success"]'),
    ).toContainText('Workflow deleted')
    await expect(
      page.locator('[data-testid^="wf-list-card-"]').filter({ hasText: slug }),
    ).toHaveCount(0, { timeout: 15000 })
  })
})
