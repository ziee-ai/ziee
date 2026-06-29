import type { Page } from '@playwright/test'
import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'

/**
 * E2E — the Hub "Workflows" tab + the install-from-hub flow.
 *
 * Audit gap: the workflow hub tab (`hub/modules/workflow/`) and its
 * install path (`WorkflowHubCard` → POST /api/workflows/install-from-hub,
 * backend-only covered by tests/workflow/install_from_hub.rs) had ZERO E2E
 * coverage — `17-workflows/list-page-renders.spec.ts` only checks the Import
 * button is visible.
 *
 * Test 1 (real path): the tab mounts and renders its search affordance.
 * Test 2 (mocked catalog): seed one workflow into the hub catalog, click
 * "Install", and assert the card flips to the green "Installed" tag after the
 * post-install `/api/hub/installed` refetch. Only the HTTP boundary is mocked;
 * the card state machine + store install action run for real.
 */

function workflowItem() {
  return {
    name: 'demo-workflow',
    title: 'Demo Workflow',
    summary: 'A demo workflow for the hub install E2E.',
    category: 'workflow' as const,
    manifest_path: 'workflows/demo-workflow/manifest.json',
    version: '1.0.0',
    tags: ['demo'],
    verified: true,
  }
}

async function mockHubCatalog(page: Page, opts: { installed: { value: boolean } }) {
  await page.route(/\/api\/hub\/index$/, route =>
    route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({
        hub_version: '1.0.0',
        schema_version: 1,
        generated_at: new Date().toISOString(),
        items: [workflowItem()],
      }),
    }),
  )
  await page.route(/\/api\/hub\/version$/, route =>
    route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({
        hub_version: '1.0.0',
        server_version: '0.1.0',
        source: 'seed',
        counts: { assistants: 0, mcp_servers: 0, models: 0, skills: 0, workflows: 1 },
      }),
    }),
  )
  await page.route(/\/api\/hub\/installed$/, route =>
    route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({
        catalog_version: '1.0.0',
        items: opts.installed.value
          ? [
              {
                current_version: '1.0.0',
                entity_id: 'wf-entity-1',
                entity_type: 'workflow',
                hub_category: 'workflow',
                hub_id: 'demo-workflow',
                installed_at: new Date().toISOString(),
                installed_version: '1.0.0',
                is_system: false,
              },
            ]
          : [],
      }),
    }),
  )
}

test.describe('Hub — Workflows tab', () => {
  test('renders the workflows tab surface', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await page.goto(`${baseURL}/hub/workflows`)
    await expect(page).toHaveURL(/\/hub\/workflows/)
    // The tab's own search input proves the WorkflowsHubTab component mounted.
    await expect(
      page.getByTestId('hub-workflows-search-input'),
    ).toBeVisible({ timeout: 30000 })
  })

  test('install-for-me flips the card to the Installed state', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    const installed = { value: false }
    await loginAsAdmin(page, baseURL)
    await mockHubCatalog(page, { installed })

    // The POST install endpoint succeeds, then we flip the installed flag so
    // the store's post-install `/api/hub/installed` refetch sees the new row.
    await page.route(/\/api\/workflows\/install-from-hub$/, route => {
      installed.value = true
      return route.fulfill({
        status: 201,
        contentType: 'application/json',
        body: JSON.stringify({
          id: 'wf-entity-1',
          name: 'demo-workflow',
          hub_id: 'demo-workflow',
        }),
      })
    })

    await page.goto(`${baseURL}/hub/workflows`)
    const card = page.getByTestId('hub-workflow-card-demo-workflow')
    await expect(card).toBeVisible({ timeout: 30000 })

    // Admin sees the split "Install" button — its main segment installs-for-me.
    await page.getByTestId('hub-workflow-install-dropdown-btn-demo-workflow').click()
    await page.keyboard.press('Escape')

    await expect(
      page.getByTestId('hub-workflow-installed-tag-demo-workflow'),
    ).toBeVisible({ timeout: 10000 })
  })
})
