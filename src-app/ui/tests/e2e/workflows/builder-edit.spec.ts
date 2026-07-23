import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import { byTestId } from '../testid'
import {
  goToWorkflowsSettingsPage,
  openWorkflowCard,
  seedDevWorkflow,
} from './helpers/workflow-helpers'
import { saveBuilder, waitBuilderValid } from './helpers/builder-helpers'

/**
 * TEST-11 — the builder EDIT flow (edit-in-place). "Edit" from the detail
 * drawer opens the builder pre-loaded with the existing definition; changing a
 * step + Save PUTs in place (same workflow id / route), and reopening the
 * drawer shows the change. No API mocking — the workflow is seeded through the
 * real import API and edited through the real builder + definition PUT.
 */

const SEED_YAML = `$schema: "/schemas/2026-06-12/workflow-definition.schema.json"
inputs:
  - name: topic
    required: true
steps:
  - id: summarize
    kind: llm
    description: "Original label"
    prompt: |
      In ONE short sentence, say something about "{{ inputs.topic }}".
outputs:
  - name: summary
    from: "{{ summarize.output }}"
    expose: full
`

test.describe('Workflows — builder edit (in place)', () => {
  test('Edit loads the def, a change persists, and the id/route is unchanged', async ({
    page,
    request,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    const slug = `e2e-builder-edit-${Date.now()}`

    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    const workflowId = await seedDevWorkflow(
      request,
      apiURL,
      token,
      slug,
      SEED_YAML,
    )

    // Open the seeded workflow's detail drawer and click Edit.
    await goToWorkflowsSettingsPage(page, baseURL)
    await openWorkflowCard(page, slug)
    await expect(byTestId(page, 'wf-detail-drawer')).toContainText(
      'Original label',
    )
    await byTestId(page, 'wf-detail-edit-btn').click()

    // The builder opens in EDIT mode on THIS workflow's id (edit-in-place route).
    await expect(page).toHaveURL(
      new RegExp(`/settings/workflows/${workflowId}/edit$`),
      { timeout: 15000 },
    )
    await expect(byTestId(page, 'wf-builder-page-title')).toContainText(
      'Edit workflow',
    )

    // The existing step is pre-loaded (its label is shown in the step row).
    const stepRow = byTestId(page, 'wf-builder-step-row-summarize')
    await expect(stepRow).toBeVisible({ timeout: 15000 })
    await expect(stepRow).toContainText('Original label')

    // Change the step: retitle it. The prompt is pre-loaded (edit did not blank it).
    await stepRow.click()
    await expect(byTestId(page, 'wf-builder-llm-prompt')).toHaveValue(
      /say something about/,
    )
    await byTestId(page, 'wf-builder-step-description').fill('Edited label')

    // Save in place (PUT). No name field in edit mode; the row updates.
    await waitBuilderValid(page)
    await saveBuilder(page)
    // Still on the SAME edit route — the id did not change (no new workflow).
    await expect(page).toHaveURL(
      new RegExp(`/settings/workflows/${workflowId}/edit$`),
    )

    // Reopen the drawer from a fresh list load → the change persisted server-side.
    await goToWorkflowsSettingsPage(page, baseURL)
    await openWorkflowCard(page, slug)
    const drawer = byTestId(page, 'wf-detail-drawer')
    await expect(drawer).toContainText('Edited label', { timeout: 15000 })
    await expect(drawer).not.toContainText('Original label')
  })
})
