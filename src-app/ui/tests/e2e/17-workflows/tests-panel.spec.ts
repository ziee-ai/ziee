import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import {
  goToWorkflowsSettingsPage,
  openWorkflowCard,
  seedDevWorkflow,
} from './helpers/workflow-helpers'

/**
 * WorkflowTestsPanel: opened from the detail drawer's "Run tests" button, it
 * loads test results on mount (POST /workflows/{id}/test → spinner) and renders
 * pass/fail/skip Tag counts + per-fixture rows. The test EXECUTION is the
 * external boundary, mocked here; the PANEL rendering is the behavior under test.
 */
const WF_YAML = `$schema: "/schemas/2026-06-12/workflow-definition.schema.json"
inputs:
  - name: topic
    required: false
steps:
  - id: noop
    kind: llm
    prompt: "about {{ inputs.topic }}"
outputs:
  - name: out
    from: "{{ noop.output }}"
    expose: full
`

test.describe('Workflows - tests panel', () => {
  test('Run tests opens the panel and renders pass/fail/skip counts', async ({
    page,
    request,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)
    const slug = `e2e-tests-panel-${Date.now()}`
    await seedDevWorkflow(request, apiURL, adminToken, slug, WF_YAML)

    // Mock the test-run result (the boundary that executes fixtures).
    await page.route(/\/api\/workflows\/[0-9a-f-]+\/test$/, async (route, req) => {
      if (req.method() === 'POST') {
        return route.fulfill({
          status: 200,
          contentType: 'application/json',
          body: JSON.stringify({
            total: 4,
            passed: 2,
            failed: 1,
            skipped: 1,
            results: [
              { name: 'fixture-ok-1', passed: true, duration_ms: 12 },
              { name: 'fixture-ok-2', passed: true, duration_ms: 9 },
              {
                name: 'fixture-bad',
                passed: false,
                duration_ms: 7,
                failure: { message: 'assertion failed' } as any,
              },
              { name: 'fixture-skipped', passed: false, skipped: true, duration_ms: 0 },
            ],
          }),
        })
      }
      return route.continue()
    })

    await goToWorkflowsSettingsPage(page, baseURL)
    await openWorkflowCard(page, slug)

    await page.getByRole('button', { name: 'Run tests' }).click()

    // The panel renders the aggregate pass/fail/skip Tag counts...
    await expect(page.getByText('2 passed')).toBeVisible({ timeout: 10000 })
    await expect(page.getByText('1 failed')).toBeVisible()
    await expect(page.getByText('1 skipped')).toBeVisible()
    // ...and the per-fixture rows.
    await expect(page.getByText('fixture-ok-1')).toBeVisible()
    await expect(page.getByText('fixture-bad')).toBeVisible()
  })
})
