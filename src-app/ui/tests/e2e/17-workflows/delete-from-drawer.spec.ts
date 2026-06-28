import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import {
  goToWorkflowsSettingsPage,
  openWorkflowCard,
  seedDevWorkflow,
} from './helpers/workflow-helpers'

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

    // The drawer's Delete button → Popconfirm "Delete this workflow?".
    await page.getByRole('button', { name: 'Delete', exact: true }).click()
    await expect(page.getByText(/delete this workflow\?/i)).toBeVisible()
    // Confirm via the danger OK button (okText "Delete").
    await page
      .locator('.ant-popconfirm:visible')
      .getByRole('button', { name: 'Delete', exact: true })
      .click()

    // Success toast + the drawer closes + the card is gone from the list.
    await expect(page.getByText('Workflow deleted')).toBeVisible()
    await expect(
      page.locator('.ant-card', { hasText: slug }),
    ).toHaveCount(0, { timeout: 15000 })
  })
})
