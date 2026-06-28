import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import {
  goToWorkflowsPage,
  seedDevWorkflow,
} from './helpers/workflow-helpers'

/**
 * E2E — Workflows list page rendered WITH data (audit gap all-495967c5c50a).
 *
 * `list-page-renders.spec.ts` only asserts the antd <Empty> state on a fresh
 * DB. This is the populated-list counterpart: it seeds a real workflow through
 * the API (`POST /api/workflows/import`, the same path the Import dialog uses)
 * and asserts the list page renders that workflow's card — proving the
 * WorkflowsList store hydration + card rendering path, not just the empty
 * branch. No mocks: the row is a real persisted workflow.
 */

const SEEDED_WORKFLOW_YAML = `$schema: "/schemas/2026-06-12/workflow-definition.schema.json"
inputs:
  - name: topic
    description: "subject to summarize"
    required: true
steps:
  - id: summarize
    kind: llm
    message: "Summarizing {{ inputs.topic }}"
    prompt: |
      In ONE short sentence, say something about "{{ inputs.topic }}".
outputs:
  - name: summary
    from: "{{ summarize.output }}"
    expose: full
`

test.describe('Workflows - List page with data', () => {
  test('renders a card for an installed workflow (non-empty list)', async ({
    page,
    request,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)

    // Seed a real workflow via the API so the list is populated.
    const slug = 'e2e-list-with-data'
    await seedDevWorkflow(request, apiURL, adminToken, slug, SEEDED_WORKFLOW_YAML)

    await goToWorkflowsPage(page, baseURL)

    // The empty state must NOT be shown now that a workflow exists.
    await expect(page.getByText(/no workflows installed yet/i)).toHaveCount(0)

    // The seeded workflow renders as a card showing its name.
    const card = page.locator('.ant-card', { hasText: slug }).first()
    await expect(card).toBeVisible({ timeout: 15000 })
    await expect(page.getByText(slug).first()).toBeVisible()
  })
})
